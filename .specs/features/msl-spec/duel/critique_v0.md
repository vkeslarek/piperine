# Critique of spec_v0 (round 0)

No previous changelog: nothing to verify as CLOSED/REOPENED.

Ranking: 3 BLOCKER, 11 MAJOR, 9 MINOR. Items marked [HOLE], [BLOAT] or [INCONSISTENT].

---

## BLOCKERS

### B1 [HOLE] Multi-driver resolution, strengths, tristate and `inout` are declared out of scope (5.4, G)
The goal lists them as required. "A shared line is a module" only works when one module can see every driver, and it breaks on the first bidirectional element.

```rust
// Two tristate buses joined by a bidirectional pass gate (Verilog tranif1).
module Board(in en: Bit, a: [flip Pin<u8>; 2], b: [flip Pin<u8>; 2]) {
    signal la: Bit<u8>;  signal lb: Bit<u8>;
    let ba = TriBus::<u8, 3>([a[0], a[1], pa]);   // third "device" is the gate
    let bb = TriBus::<u8, 3>([b[0], b[1], pb]);
    // gate: pa.o = lb, pa.oe = en & (lb driven);  pb.o = la, pb.oe = en & (la driven)
}
```
- `la` depends on `lb` and `lb` on `la` with no reaction between: an elaboration error by 8.2 rule 3. The circuit is legal hardware.
- "Driven or not" is not observable: `TriBus` returns `Unknown` both for zero drivers and for contention, so a pull-up, a bus keeper, a `trireg` and a weak-versus-strong fight cannot be told apart.
- No `inout` on `Bit`: every level of hierarchy between a pad and the bus must thread three wires (`o`, `oe`, `i`) and the bus module must sit at the common ancestor with `N` fixed. A driver cannot be added from a sibling subtree without editing the ancestor.
- Importing any netlist with `inout`, `$tribuf` chains, or `tran` (G.8) is impossible by construction.

Fix without adding a concept: **a nature states how its drivers combine.** Continuous natures already do (flow sums, KCL). Give an enumerated nature an optional `resolve: fn(&[Drive<T>]) -> T` (fold over the connected component); default absent means the single-driver rule. Z is "no drive" (`Option`), strength is a value field in `Drive<T>` defined in the stdlib. This deletes `TriBus`, `OpenDrainBus`, `WiredOr`, `SumNet`, `MaxNet`, `MinNet`, `AvgNet`, `Pin`, and closes G.25 (user enumerated natures) with the same rule. A pass gate is then a conditional `=` between two resolved carriers, or stays analog; state which.

### B2 [HOLE] The re-solve at the same `t` has no defined equations (15 step 3.4, 17.2)
"Solve again at `t` and restart the integrator" does not say what `ddt` is during that solve, nor which quantities are continuous across the jump. Every Bit-to-analog discontinuity depends on it.

```rust
module ShareCharge(a: Electrical, b: Electrical, in close: Bit) {
    let c1 = Capacitor(a, gnd) { c: 1p };      // a at 1 V
    let c2 = Capacitor(b, gnd) { c: 3p };      // b at 0 V
    let sw = Switch(a, b, close);              // 11: ideal short when close
}
```
At `close.rise()`: if `ddt = 0` in the re-solve (an equilibrium solve), both nodes float and the matrix is singular. If node potentials are held, the short `v(a,b) = 0` contradicts them. The physical answer (0.25 V, charge conserved) needs "the arguments of `ddt` are the continuous states; a re-solve conserves them across the component unless a potential constraint forces a jump, and then the flow is impulsive". Nothing of the sort is written. The same hole decides `ddt(c * v)` when `c` is a `Bit<f64>` that jumps, and `DigiPot`, `Dram1T1C` (wordline), `Water` (phase change holds `temp` while `ddt` terms vanish from the arm).
Required: one rule for the right-limit solve (what is unknown, what is held, what `ddt`/`delay`/`transition`/`idt` state does), and what "restart the integrator" keeps.

### B3 [HOLE] Unknown-ness of *values* is undefined; X semantics exist only for carriers (5.2 vs Part I)
Part I says values are plain Rust data. 5.2 gives X rules for operators on carriers. A reaction reads a carrier into a local and the spec stops.

```rust
on fref.rise() || fvco.rise() {          // Pfd3, 13: `state` is unknown at start in a variant with no init
    let mut s = state;                   // is `s: i2` unknown? i2 has no unknown value
    if fref.rise() && s < 1 { s += 1; }  // an `if` that neither yields nor commits: which 5.2 row?
    state <- f(s);                       // does X pass through a user fn? through `as f64`? through a closure?
}
```
- Either every value type secretly carries a per-level unknown mask (then "values follow Rust", `Repr::encode -> [bool; W]`, `decode -> Option`, pure `fn` on values, and "read inside `analog` is an error" all need restating), or a read yields `Option<T>`/a 4-state type (then `count + 1`, `if rst_n`, `match st` in every gallery do not type-check).
- `match` on an unknown takes "the `_` arm": `UartTx` (E.4) has only a guarded `_ if !baud_tick`; exhaustive enum matches have none. Undefined.
- "`if` that yields merges, `if` that commits takes `else`" makes `q <- if c {a} else {b};` and `if c { q <- a; } else { q <- b; }` differ, and leaves `if c { s += 1; }` (a local mutation) uncovered. Three cases, two rules.
- `TriBus` relies on `.filter(|d| d.oe).count()` with an unknown `oe`; iterator closures over unknowns are unspecified.

Pick one rule and write it once: e.g. "a read of `Bit<T>` is a value of `T` plus a known-mask; every operator is defined level-wise (5.2 row 1) or all-or-nothing (row 2); control flow on an unknown condition executes both arms and merges every effect (locals and commits) level by level". That single rule removes the yields/commits split and the `_`-arm row.

---

## MAJOR

### M1 [HOLE] The stdlib `idt` and `idtmod` are wrong as written, so "everything else is stdlib" is unproven (8.3, 22.2)
```rust
analog for idt { ... else { i(y) +> ddt(v(y)) - x; } }
```
- In every equilibrium solve `ddt = 0`, the row is `-x = 0`, and `v(y)` appears nowhere: singular matrix. `Water`, `PiAw`, `Vco`, `Pll` cannot find an operating point. Needs `if analysis(Static) { v(y) +> ic }` (VAMS 4.5.4 semantics), and the no-`ic` feedback case stated.
- The state lives on `Electrical` (abstol 1 µV / 1 pA) whatever `x` is: `Water` integrates joules up to 2.26e6, `Vco` integrates cycles. Tolerances are meaningless; `idt` needs the nature (or `abstol`) as a parameter. Same for `Nullor`: residual in volts tested against 1 pA.
- `idtmod` keeps an unbounded `acc` and subtracts `k * modulus`. After 1 s at 1 GHz `acc = 1e9`; `reltol · |x|` on that unknown is 1e6 cycles. That is exactly the failure `idtmod` exists to avoid. It must re-seat the integrator state at each wrap (force `v(y)` for one microstep through the switch branch), which in turn depends on B2.

### M2 [HOLE] The collision rule turns legal coincidences into run-time errors; transport `after` has no overtaking rule (12.1, 8.2 rule 4)
```rust
on fref.rise() { up <- true; }                                  // Pfd, F.2
on (...) && up && dn { up <- false after treset; ... }          // lands on a 1 fs tick
```
With integer ticks, `fref` rising at the tick where the delayed `up <- false` lands is an ordinary event; the spec kills the run. Same shape in `Front`/`A2D`/`Bounded` (set and clear in separate reactions). Since 5.4 guarantees all writers are in one module, a deterministic rule is free: later reaction in source order wins, a delayed commit loses to a same-instant immediate one (or the reverse; pick). Keep the error only as a lint.
`phi1 = clk after if clk { gap } else { 0.0 };` (D.6): `clk` falls before `gap` elapses, `phi1` falls now, then the earlier scheduled rise lands and `phi1` sticks high with `clk` low. "Every scheduled commit happens, in order" needs the VHDL rule (a new transaction deletes pending ones at later-or-equal times) or the gallery is wrong. Also unspecified: two commits scheduled at different times for the same future instant.

### M3 [HOLE] Time-0 consistency of event-driven logic (16, E.2)
```rust
digital<T> for DLatch<T> { on en.changed() || d.changed() { if en { q <- d; } } }
```
`en = true`, `d = 5` initially, both by `= value` or host drive at t = 0: nothing "changed", `q` stays unknown forever while the latch is transparent. Definitions settle at the operating point, reactions do not. Either `changed()` fires at `(0,0)` for every carrier that leaves unknown (Verilog's X-to-value at time 0), or every level-sensitive model needs `init ||`, which no gallery model has. Also unspecified: a parent `signal up: Bit;` (unknown) wired to a child `out up: Bit = false`: which initial value holds.

### M4 [HOLE] Digital state across non-transient analyses is unspecified (11.1, 13, 16)
```rust
digital for A2D { on above(v(a) - vth - hyst/2) { d <- true; }  on above(vth - hyst/2 - v(a)) { d <- false; } }
```
DC sweep of a Schmitt trigger: does `Bit` state persist from one sweep point to the next (hysteresis loop) or reset to `= value`? Is each sweep point an "initial solve" for `above` and `init`? What do `at`, `after` and `settle` do in `Dc`/`Ac`/`Noise` (`ClockGen` schedules `after` from `init`)? Which Bit values does the AC linearization see when the OP fixed point oscillates? One paragraph: "in every analysis without time, only the fixed point of 16.4 runs; pending timed commits are discarded; state carries between sweep points".

### M5 [INCONSISTENT][BLOAT] Three kinds of callable, and "pure `fn`" is false (3, 8.3, 14, B.2, 22.2)
- 3: a function is pure and stateless. 14: `ddt`, `delay`, `transition`, `slew`, `limit`, `noise`, located events "keep state per call site". B.2: these are not allowed in `fn`.
- 22.2 then writes `fn limexp` (calls `limit`), `fn white_noise`/`flicker_noise` (call `noise`), and 3.1 writes `fn reaches` (calls `above`). All violate B.2; all are stateful functions.
- So there are pure fns, call-site-stateful primitives, and modules with a result, with three sets of placement rules (B.2 rows 7, 8, 9).

Merge: every stateful primitive is an (extern) module with a result; "every stateful call is an instance" (8.3 rule 3) becomes the only rule; a `fn` that contains one is not a `fn`. Then `limexp`, `white_noise`, `reaches` become modules with a result or are inlined by a stated rule. Deletes B.2 rows and the "per call site" sentence of 14.

### M6 [HOLE] Value ports of a module with a result are call-by-name, an unlisted concept (8.3 rule 1)
`module last_crossing(e: f64, dir) -> f64` uses `cross(e, dir)` in its own `digital` block: `e` is the caller's *expression*, re-evaluated by the solver while locating. `idt(x)` re-reads `x` per iteration and differentiates through it. Rule 1 gives two evaluation regimes ("every iteration when they probe, otherwise when inputs change"). This is neither a value nor a carrier. Either say "a value port is a signal-flow input node the caller drives with the expression" (then it is concept 3, and `reset: Option<bool>` is a `Bit` definition), or list it as a concept. As written concept 5 hides it. Also `table_model` and `noise_table` are listed as modules with a result though stateless.

### M7 [HOLE] Small-signal response of sampled state is lost (22.2, 5.1 rule 2)
`zi_nd`/`zi_zp` are "a sampled `Bit<f64>` state per delay"; a `Bit<f64>` "has no derivative". In `Ac`/`Noise` their transfer is therefore zero, while VAMS 4.5.12 defines `H(z)` at `z = exp(jωT)`. A decimation filter or a discrete-time loop filter has no AC response. Either `zi_*` is core (extern module with a result) or the language needs a way for a module to state its small-signal transfer (`if analysis(Ac) { … delay(x, T) … }` works for `zi` if `delay` composes: show it).

### M8 [HOLE] Switch-branch selector is unconstrained (11 rule 2)
"A branch that gets potential in some iterations and flow in others" allows
```rust
if v(a, b) > 0.0 { v(a, b) +> 0.0; } else { i(a, b) +> 0.0; }   // ideal diode
```
The matrix structure flips inside one Newton solve; convergence is undefined and the complementarity never settles (at the short `v = 0`, so the test is false, so it opens). Rule needed: the selector of a branch's kind is constant during a solve (params, `Bit`, `analysis`); a probe-dependent one is an error. State also whether `v(a, ai) +> 0.0` really collapses the node (Diode) or adds an unknown.

### M9 [INCONSISTENT] 17.2 rule 2 contradicts the table below it
Rule 2: "every known time is landed on exactly: `at`, `after`…". Table row 1 and D.2: a `Bit` change nothing analog reads is no step boundary and "costs the solver nothing". Both cannot hold. Also 12.1 rule 5 (a probing reaction makes a boundary) versus `on settle` probing (no boundary).

### M10 [BLOAT] Core primitives that are stdlib in this very language (13, 14, G.26)
- `above` = `cross(e, Rising) || (init && e > 0.0)` once 13 rule 6 (sign change between two solves of one instant fires) is applied to the rounds of 16.4. Delete; G.26 already concedes.
- `absdelta` = a module with a result holding `last: Bit<f64>` and `on cross(e - last - delta) || cross(last - delta - e) || init { last <- e; }`. The "needs step control" claim (14) is false.
- `transition` = the `Pwl` pattern of D.1 (a `Bit` segment state, `at` corners, a ramp from `now()`), if its input is a `Bit<f64>`. It is only core because it accepts a non-carrier `x` whose changes it cannot observe; all gallery uses pass Bit-derived values.
Three fewer primitives; `slew`, `delay`, `ddt`, `limit`, `noise`, `ac_stim`, `ddx`, `bound_step`, `discontinuity` stay.

### M11 [BLOAT] Views are a third meaning of `=` and an exception to the definition rule (5.5, 8.2 rule 6)
`byte = sh;` is a definition whose expression is a reinterpretation. Delete 5.5 and rule 6; add stdlib `fn cast<A: Repr, B: Repr>(a: A) -> B where A::WIDTH == B::WIDTH`. `byte = cast(sh);`. "Read-only view, writes go to the source" then follows from 5.4 with no extra text.

---

## MINOR

- **m1 [BLOAT] Attributes.** 13 attributes. `#[opvar]`: 21 says the host observes everything, so a named `let` in `analog` is already readable; delete. `#[nondiff]`: inferable (param reaches only `Bit` logic, `match`, generation); delete. `#[mc]` and `#![time_resolution]`: host/session concerns by Part VIII's own argument; delete. `#[units]` duplicates nature `units`; `#[alias]` is import sugar. `#[osdi]` + `#[device]` are one `#[extern(kind, …)]`. `in range` on params is admitted sugar for `where` (8.1); delete. Target: 5 attributes.
- **m2 [BLOAT] `interface` is a struct of carriers.** Allow carrier-typed fields with directions in `struct`; drop the keyword, the `module Name: Interface` head (use one port `c: Cell`; bound `C: Cell` = module whose port list is that struct) and G.4/G.18 shrink.
- **m3 [BLOAT] Parent contributing into a child's `pub` branch** (8.2 rule 3) is an exception to "a module writes its own". The example is not even consistent: `Inductor` (11) contributes to the unnamed `(p, n)`, not to a `pub br`. A mutual inductance is a flux port (a `Magnetic` nature) on the inductor; no reusable `Mutual(l1, l2)` can be written under the current rule anyway (no instance-reference params). Delete the write half; keep read.
- **m4 [INCONSISTENT] Grammar.** B.3 `port ::= dir? IDENT ':' sigtype` puts `flip` before the name; 7 and 22.3 write `rx: flip Stream<T>` and `[flip Pin<T>; N]`. `quantity ( ',' quantity )?` versus attribute blocks. `after dt > 0` (12.1) versus "0 is no delay" (8.2 rule 4).
- **m5 [HOLE] Tick rounding of located crossings** (15): rounding down puts the reaction at a time where `e` has not crossed and the probe reads the wrong sign; say "rounded up" and that the solution is re-evaluated at the tick. `held()`/`since()` return `f64` while time is integer.
- **m6 [HOLE] Host `set` of a param that feeds generation or a const generic** (`for k in 0..n`): error, or re-elaboration? 9 says only "values computed from params are recomputed".
- **m7 [HOLE] Unknown `Bit` read by `analog` is a run-time error** (5.2), yet most carriers start unknown (16.2): `DacRamp(code)`, `Switch(ctl)`, `ChargePump` fed by an un-reset `Dff` abort at the operating point. `D2A` handles it by hand with `level(0)`. Either the error is at elaboration (an `in` Bit read by `analog` must be read through `known_or`) or the default is defined.
- **m8 [HOLE] `#[device]` cosimulation contract** is one sentence (8.5); how an extern digital model's outputs enter the microstep loop, and rollback (17.2 rule 3) across a foreign simulator, are undefined. OSDI opvars/noise names/internal nodes exposure is unstated.
- **m9 [HOLE] Per-carrier history on memories.** "Every `Bit` carrier keeps one previous value and two times" on `[Bit<u8>; 1 << 32]`; sparse storage was removed (H). State that history and storage are materialized on demand, or large memories are not simulable.

---

## Counts (spec_v0)

**Concepts: 10 claimed, 19 effective.** The table omits what the text treats as separate rules: `Repr`; param (with host `set`, history, checks); node/branch (a second carrier kind with switch branches); module with a result; value port (call-by-name, M6); template; extern module; generation; view; host. Concept 10 ("input and output") is two primitives, not a concept.

**Constructs: 72** by B.1 (14 keywords incl. contextual and reused; 2 operators; 2 call rules; 2 literals; 8 built-in types/traits; 6 carrier methods; 7 events; 11 analog primitives; 5 environment; 2 output; 13 attributes). Unlisted in B.1 but used: `gnd`, `Node`, `self` on modules, struct-update on templates.

Reachable next round if the proposals are taken: B1 (−9 stdlib modules, −`Pin`, closes G.25, +1 nature field), M5 (−2 callable kinds), M10 (−3 primitives), M11 (−1 rule), m1 (−8 attributes, −`in`), m2 (−1 keyword, −1 module head), m3 (−1 rule): about **55 constructs, 12 honest concepts**.

VERDICT: CONTINUE

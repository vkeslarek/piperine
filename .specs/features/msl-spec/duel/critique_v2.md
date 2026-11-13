# Critique of spec_v2 (round 2)

Ranking: 1 BLOCKER, 3 MAJOR, 8 MINOR. Tags: [HOLE], [BLOAT], [INCONSISTENT].
Galleries and stdlib were traced by hand against the rules, not compiled (no compiler exists).
Elegance is scored on what the language adds to Rust; Rust's own value-level syntax is not penalized.

---

## Verification of changelog_v2 against critique_v1

| v1 item | Status | Note |
| --- | --- | --- |
| M1 `settle` commit read by `analog` | CLOSED for `analog`, residue → M2 | "Arrive" step is clear: no left/right pair, no jump, no restart. What the commit does to `changed()`, reactions and digital-only instants is not said. |
| M2 time 0 depends on who supplies the value | CLOSED, residue → M1 | One rule for model and host; `Dff`, `DLatch`, `ClockGen`, `Vco` traced and right. `init()` as a 3-line module is right. But it fires at microstep 0 only, and the stdlib seeds from it. |
| M3 delayed definition vs delayed commit | CLOSED as a rule, residue → m2 | `Ring3` traced through `(0,0)` and three `tpd`: starts. `SrLatch` holds. The rebuttal of "next round" (clock generators oscillating inside DC) is accepted. |
| M4 cycles through `others()` | CLOSED, residue → m3 | `Tgate`, a 2-`Tran` ring and the F.4 table traced: settle, nothing latches. The claim for two `Bidir` pads is false (m3). |
| M5 net `=` | CLOSED | See "Net `=`" below. |
| M6 macros | CLOSED, one leftover → m5 | Six one-line functions over `emit!`; rebuttal of `macro_rules!` accepted. `matches!` is still used in 22.1. |
| M7 concept table | CLOSED, one rider → m7 | Drive, `let`, instant have rows; host and output are out with a reason. |
| m1 gallery | CLOSED except `zi_nd` seeding → M1 | `DacRamp`, `Pwl`, `Vco`, `nch` right. |
| m2–m11 | CLOSED | m4: rule and C.2 agree. m9: `Pfd` and `TwoPhase` traced under per-statement transport: right. m8: `slew` traced through Track → Up → Track: right. m11: right in a transient only (m2). |

Builder's weak points:
1. **A carrier that starts high has risen at `(0, 0)`** — accept. It is IEEE 1364, it is one rule, and the opt-out (`s.prev().is_known()`) is one guard. Say once, next to `Counter`, that a clock that starts high counts at time 0.
2. **Delayed logic frozen where time does not advance** — accept the rule (it is also what VAMS does: only time-0 events run in DC). Not acceptable as it stands in the stdlib: see m2.
3. **Non-settling relaxation ends in unknown, not an error** — unknown is the right value. Silent unknown is not: see m3.
4. **`target(name, margin)` written by the author** — accept. Better than a macro that splits a comparison; the margin's sign convention is one sentence. No action.
5. **`known_or` on own state** — accepted last round; `high()` helps.
6. **`transition` and `zi_nd` grew** — the growth is a symptom, and both are still wrong at time 0: see M1.

**Net `=` deleted (ports / `Tran` / `Short`)** — readability did not suffer. "`=` only defines" removes the worst trap of v1, the 0 table is three rows, and D.6/F.2/F.4 read the same as before. Two small costs: `Short` is written for `Electrical` only (make it `Short<N: Nature>` with `potential(a, b) +> 0.0`), and `Tran(a, b, true)` leaves two carriers with two `changed()` rather than one net (fine; say so).

---

## BLOCKER

### B1 [HOLE][INCONSISTENT] `idt` with an initial condition has zero small-signal response (8.3, 11.1, 16.2 row "a branch's kind")
```rust
analog for Pi { v(outp) +> k1 * v(inp) + k2 * idt(v(inp), 0.0); }     // 8.3's own example
// idt:  let hold = reset.unwrap_or(false) || (ic.is_some() && analysis(Static));
//       if hold { potential(y) +> ic.unwrap_or(0.0); } else { flow(y) +> ddt(potential(y)) - x; }
```
At the operating point of an `Ac` analysis `analysis(Static)` is true, so `y`'s branch is a potential source of value `ic`. 16.2: in the sweep a branch has "the kind it had at the operating point", and `analog` is "linearized, not executed". The linearization of `potential(y) +> 0.0` is `y = 0`. So `Pi` has no integral term in AC, `Water`'s `absorbed`, `idtmod` (always passes an `ic`) and every `laplace_*`/loop filter built on `idt(x, ic)` are flat. VAMS 4.5.4: `idt` is `1/jω` in small-signal whatever `ic` is. No model text can work around it: any constraint that exists only in equilibrium is written as a switch branch on `analysis(Static)`, and that kind is frozen into the sweep. The summary under 16.2 even says "`idt` holds or is free" for all three columns.
Fix, one sentence, and it deletes a row: *the sweep linearizes the equations `analog` would execute with the sweep's own flags (`analysis(Static)` false, `ddt → jω`) at the operating point's values and discrete state.* The branch-kind row then follows from the rule instead of contradicting `idt`.

---

## MAJOR

### M1 [HOLE] The stdlib seeds its state at `init()`, which is microstep 0, not the operating point (13 rule 6, 16 step 4, 22.2 `transition`, `zi_nd`, 22.4 `D2A`)
```rust
module Top(in clk: Bit, in rst_n: Bit, in d: Bit, a: Electrical, vdd: Electrical) {
    signal q: Bit;                                  // no initial value
    let ff  = Dff(clk, rst_n, d, q) { reset: false };   // rst_n low from the start
    let pad = D2A(q, a, vdd);
}
```
Trace: `(0,0)` `q` unknown, `D2A`'s argument is 0.5; `start` fires, `y0 = y1 = 0.5`. The reset reaction commits `q` at `(0,1)`; the argument becomes 0.0; `xd` follows at `(0,2)`; now `start` is false, so `from = y = 0.5`, `to = 0.0`, `dt = tr`. The operating point is solved with the pad at `vdd/2`, and the transient opens with a ramp from mid-rail. A `Dc` analysis of the same design gives 0 (the `!analysis(Tran)` pass-through). So `Dc` and the transient's own operating point disagree for every registered output behind a `D2A`, against 16's "an analog equilibrium and a digital state that agree". `zi_nd` has the same fault: `on init()` reads `x` and `pos(y)` from the first solve, before the rounds of step 4 change them.
Root cause: there is no way to say "the operating point has gone quiet"; `init()` says "the first microstep". It needs no primitive: the first `settle` is that event, and its commit applies before the first step (15 step 1).
```rust
pub module started() -> bool { signal s: Bit = false; s.high() }     // false through the whole operating point
digital for started { on settle { s <- true; } }
```
`transition`: `let y = if !started() || !analysis(Tran) { x0 } …` and `if !started() { from = to }`; `zi_nd`: seed on the first `settle`. Both get shorter. Also state what `analysis(a)` returns inside a reaction for the solve-scoped variants (`Static`, `Ic`, `Nodeset`): 14 allows it "everywhere" and 11.1 defines it per solve.

### M2 [HOLE] A `settle` commit's effect on the digital side is undefined (13 table, 15 steps 1 and 4.2, 5.3)
```rust
module PeakAlarm(a: Electrical, out over: Bit = false) {
    lim: f64 = 1.0;
    signal peak: Bit<f64> = 0.0;
    over = peak > lim;
}
digital for PeakAlarm {
    on settle { peak <- peak.max(v(a).abs()); }
    on over.rise() { emit!("alarm", at = now()); }      // ever?
}
```
15 step 1 applies the commit and "settles the structure" outside the microstep loop; `changed()` is defined as "changed in this microstep" and 4.2 fires it for changes "here or in step 1 [of the loop]". Read literally, `peak.changed()` and `over.rise()` never fire, `prev()` moves silently, and the alarm is dead. If they do fire: at which `t` (the old time point, or the next, whose time depends on the step and on rejections), in which microstep, and may their commits be read by `analog` (then it is a jump after all)? A digital-only instant inside `(t_k, t_{k+1})` reads the old `peak` while the analog step over the same interval was integrated with the new one. Pick one: the commit applies at `(t_{k+1}, 0)` *before the solve*, fires `changed()` there, and its reactions run in that instant's ordinary loop; or it is silent and the text says "a `settle` commit fires nothing". Related and also unsaid: when `settle` fires in a design with no analog unknowns (Appendix E "none of these touches the solver"; `Dut`-style checks and `on final` after "the last `settle`").

### M3 [HOLE] A node's `= value` does nothing without `uic`, and the galleries then have no operating point (6, 16 step 5, D.5, F.2)
```rust
signal x: Electrical = 0.0;
i(x) +> ddt(c * v(x)) - (v(vin) - fb) / r;      // D.5: in equilibrium the row is (v(vin) − fb)/r = 0; v(x) appears nowhere
```
6 calls `= value` "SPICE ic". 16 step 5 gives it an effect only "under `uic`". Without `uic` the integrator node of `DeltaSigma1` and `vc` of `Pll` (a charge pump into capacitors) are structurally singular at the operating point, and 16 step 3 forbids a continuation that changes the answer. The two flagship mixed-signal examples cannot start. Also undefined: what `Analysis::Ic` is relative to the operating point of 16 ("the initial-condition solve before a transient": is the ordinary transient operating point one?), whether `uic` skips the solve, and how a floating capacitor's own initial voltage is given (a branch, not a node: `if analysis(Ic) { v(p, n) +> ic } else { i(p, n) +> ddt(…) }` works only once `Ic` is defined).
One rule, SPICE's: in the operating point of a transient a node with `= value` is held at it (a potential source through the solve, released at the first step, so it is a state of that step); elsewhere it is where Newton starts. `uic` is then "no solve; unknowns without a value start at zero". `Ic` becomes derivable (m6).

---

## MINOR

- **m1 [HOLE] 16.2 is not complete.** Missing rows: the probes (complex in the sweep) and `p.inflow()`; `now()` (its value in an equilibrium solve and in a sweep); `freq()` outside a small-signal sweep (`flicker_noise` writes `k / freq().powf(exp)`: say that `noise`'s `psd` is not evaluated where `noise` is 0, or `freq()` = 0 makes it non-finite and 17.1 fails the solve); `temperature()`, `simparam`; `delay` for `t < td` (VAMS: the operating-point value); a commit with no delay or `after 0` (it does apply in equilibrium, unlike the `d > 0` row). Missing columns: the jump solve (only `ddt` mentions it) and the `Ic`/`uic` solve (M3). `Analysis` lists `Tf`, `Sens`, `Pz`, `Sp`, `Disto`, `Pss`, and "one table closes the semantics" maps none of them: `Tf`/`Sens`/`Pz`/`Sp` are column 3; `Disto` is not a linearization; `Pss` is not any of the three. Map each or drop the variant.
- **m2 [HOLE] `inertial` is unknown in every analysis without time (22.5, 16.1 rule 3).** `y` has no initial value and is written only by `on at(seen + tpd)`, which never fires in `Dc` or in the operating point of `Ac`. `TribufDelayed` therefore drives an unknown `Logic` and the `D2A` behind it sits at mid-rail: not frozen, wrong. A delayed *definition* cannot opt out either, because `analysis` is barred from definitions (14), so every `q = e after tpd` gate in a library is transient-only. `analysis`, `temperature` and `simparam` are constant during an analysis: allow them in a definition (bar only `now()`, `freq()` and probes), and give `inertial` the pass-through `transition` has.
- **m3 [INCONSISTENT] Relaxation (8.2 rule 3, 22.3, C.2, changelog M4).** (a) 22.3 promises a fixed point "in at most as many passes as there are gates on the loop"; C.2 bounds passes at 64. A ring of 100 pass gates goes unknown. The bound must come from the loop (number of drives in the component), not a constant. (b) "Two `Bidir` pads on one wire are a legal loop": traced, pass 1 both see `Z` and drive `strong(seen)`; pass 2 both see a strong drive and release; pass 3 as pass 1. It never settles, the wire is unknown, `driving` is `known_or(Strong) != HighZ` = true, and both pads drive their nodes to mid-rail. The monotonicity argument holds for `Tran`, not for a drive that drives *when others do not*. Say that such drives do not belong in a loop, or do not claim the case. (c) A zero-delay oscillation now has three answers: elaboration error (one-driver loop), run-time error (microstep loop never quiet, 15), unknown (resolved loop). Keep unknown, but write a record on `log` naming the loop; an unknown bus with no trace is the hardest bug this language can produce.
- **m4 [INCONSISTENT] Stdlib and gallery violations.** `Bit::rise`/`fall` use `matches!` (A.1: the only `!` forms are `emit!` and `include_str!`): write the `match`. `last_crossing` returns the bare carrier `t`, so it cannot be read in `analog` (3.3 rule 5), which is where VAMS uses it: return `t.known_or(-1.0)`. `Short` is `Electrical`-only. `cast`'s `where A::WIDTH == B::WIDTH` on a `fn` is in B.3 but not in B.1 (the forms row lists `where` for modules only).
- **m5 [BLOAT] Four cuts.** `final`: `on settle { target("ipk_lim", 10m - ipk); }` and the host keeps the last record, or reads `ipk` after the run (21 lets it read everything); `final` exists for one gallery line. `Analysis::Nodeset`: 18 says nodesets are hints and "not model text", then the model can branch on them. `Analysis::Ic`: once M3 is fixed it is `analysis(Tran) && analysis(Static)`. `at`'s `period`: `SpiMaster` already shows the re-armed form; `every(t0, period)` is a 4-line stdlib module, and rule 4 of 13 loses a clause.
- **m6 [INCONSISTENT] Two ways to hand an expression to an instance.** `transition(in x: Bit<f64>, …)` takes an anonymous *definition* (discrete, may not probe, exempt from 3.3 rule 5); `slew(x: f64, …)` takes a by-name *value input* (may probe, differentiated). Same call syntax, the callee's port kind decides, and `transition(v(a), …)` is an error the caller cannot predict from the call. It is principled (discrete vs continuous); 8.1 should say it in one line next to the port table, and 8.3 rule 1 should name the contrast.
- **m7 [BLOAT] Concept table riders.** Row 5 hides *template*: "a module with its params bound is a value", callable, with an `Fn(P)` bound, is not decided by "a value fixed during a solve". Row 7 lists "the core operators" under `let`; they are modules (row 6, 8.3 rule 6). And "inside a block `let` is Rust's local, the same rule read in one place" is true of immutable `let` only; say `let mut` is a value-level local and nothing more.
- **m8 [HOLE] Small unsaid things.** Is recursive instantiation in generation allowed (a tree adder, `if N > 1 { let l = Tree::<{N/2}>(…); }`)? `prev()` on a resolved carrier after relaxation: the previous *settled* value, presumably. `Setup`/`MinPulse` measure from the `(0, 0)` commit (as 1364 does); the sentence under 22.5 ("none fires merely because the run started") is true for microstep 0 only.

---

## Elegance score

| Axis | v1 | v2 | Justification (v2) |
| --- | --- | --- | --- |
| 1. Orthogonality | 6 | 8 | `=`, `+>`, `<-` each mean one thing; value input and result are `let`; left: two expression-passing regimes by port kind (m6), `at` period vs re-arm, `i(p)` vs `inflow()`. |
| 2. One rule per concept | 6 | 7 | Time 0, `after`, transport, `settle`→`analog` are single rules now; exceptions left: branch kind in the sweep (B1), `settle` commits and `changed()` (M2), three answers to a zero-delay loop (m3), "environment everywhere but definitions" (m2). |
| 3. Syntax economy | 6 | 7 | 66 honest, no macros, no lints, three attributes; `final`, two `Analysis` variants and `at`'s period do not pull their weight (m5). |
| 4. Stdlib-in-language | 7 | 8 | `init`, `since`, `slew`, `inertial`, `idt`, checks and resolved nets are all written out and mostly trace; three of them are wrong for want of a stdlib event, not a primitive (M1, m2), and one uses a macro (m4). |
| 5. Readability | 6 | 8 | No `init \|\|`, `high()`, checks as calls, a three-row write table, models read like the circuit; `transition`/`zi_nd` are hard to audit, and start-high edges need one stated caveat. |
| 6. Semantic closure | 6 | 7 | 16.2 is the right device and closes most of v1; open: `idt` in AC (B1), the operating point vs `init()` (M1), `settle` commits on the digital side (M2), node initial conditions (M3), unmapped analyses and missing rows (m1). |
| **Mean** | **6.2** | **7.5** | |

Changes that raise the lowest axes most:
1. **Closure, one rule:** state the small-signal rule as "linearize what `analog` would execute under the sweep's flags, at the operating point's values" (B1), then finish 16.2 with the missing rows, the jump and `Ic` columns, and a column assignment for every `Analysis` variant kept (m1).
2. **One rule, closure:** define the operating point's edges once: node `= value` held through a transient's operating point (M3); a `settle` commit fires `changed()` at the next time point's microstep 0, or fires nothing (M2); stdlib `started()` on the first `settle` replaces `init()` as the seed in `transition`, `zi_nd`, `inertial` (M1, m2).
3. **Syntax economy:** delete `final`, `Analysis::Ic`, `Analysis::Nodeset` and `at`'s period; allow analysis-constant environment calls in definitions instead of forbidding all of them (m5, m2).

## Counts (spec_v2)

**Concepts: 11 claimed, 12 effective** (the eleven rows + template, which the param rule does not decide; m7).

**Constructs: 66 claimed (B.1), 67 honest** (+ const-expression `where` on a `fn`; `matches!` is used but should be deleted, not counted). Reachable next round: − `final`, − `at` period (a rule clause, not a token), `Ic`/`Nodeset` out of the enum: **about 66 with the `fn` `where` listed, 11 concepts** once template is stated as part of the module rule or the param rule is widened honestly.

VERDICT: CONTINUE

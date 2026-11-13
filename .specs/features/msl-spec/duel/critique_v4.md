# Critique of spec_v4 (round 4, final verification)

Ranking: 0 BLOCKER, 2 MAJOR, 6 MINOR. Tags: [HOLE], [BLOAT], [INCONSISTENT].
Method: word-level diff of v3 → v4 read in full; the changed sections (0, 3, 11.1, 13, 14, 15, 16–16.4, 17.2, 19, 22.1, 22.2, 22.5, 23, B.1, D.2, D.4, G) re-read whole; the rest skimmed, it is unchanged since the v3 audit. Stdlib traced by hand, not compiled (no compiler exists).
Elegance is scored on what the language adds to Rust.

Both MAJORs are in text that is new this round (16.4 case 2, periodic small-signal). Nothing that was sound in v3 regressed.

---

## Verification of changelog_v4 against critique_v3

| v3 item | Status | Note |
| --- | --- | --- |
| M1 continuation between analyses | CLOSED for case 1, new hole in case 2 → M1 | "Continue a transient" is exactly right and complete: queue, state, history kept; no operating point; `init()` silent; `analysis(Static)` never true. `ClockGen`, `transition` mid-ramp, `every` traced: all keep running. Case 2 ("start from a final state") breaks when the new analysis is a `Tran`. |
| m1 `starting()` | CLOSED | Zero `starting()` / `now() == 0` left outside the history rows (grepped). `transition`, `zi_nd`, `inertial` traced with `analysis(Static)`: transient operating point, `Dc` sweep, `Ac` sweep (`d == 0`, `y = y1 = x`), continued transient (never true, nothing re-seated). No behavior changed. |
| m2 `Static` regime | CLOSED | Said once in 11.1; the tables of 3 and 14 split on the same line; 8.2 rule 5 and B.2 agree. |
| m3 periodic small-signal | CLOSED as a refusal, REOPENED as a definition → M2, m1 | It is now defined, and the definition is wrong for the canonical case (a sampled real value). |
| m4 `transition` unknown start | CLOSED | Traced: unknown `x` at the operating point gives 0.0; first known value at `t1` reaches `xd` at `t1 + td`, ramps from 0.0 over `tr`. Known start: the `td` echo sees `to == y`, no-op. D.4's sentence is true. |
| m5 `settle` corners | CLOSED | "what `(t_next, 0)` will apply, whichever commit wins, and the definitions that follow". Step 3.4's parenthesis reads backwards (m4 below), the rule itself is right. |
| m6 hint vs hold | CLOSED, residue → m2 | Stated plainly. The conflict case covers only a hold that *contradicts*. |
| m7 leftovers | CLOSED | `log` instance-relative, 65 stands; 3's table and 11 rule 3 fixed. |
| m8 repetition | CLOSED for the two rules named | See "Is the repetition gone?" below. |
| m9 unsaid | CLOSED | Stop-time `settle` commit, `since` readable in `analog`, held algebraic node. |
| G.32 opt-out | CLOSED | |

### The builder's three weak points

1. **Periodic small-signal leans on 23.3** — real, and worse than stated: "the mathematics is the same" is true, and 23.3 rule 4 is wrong in the same place for both uses. → M2. That it is unimplemented and has no gallery model is acceptable for a language spec; that it is misdefined is not.
2. **16.4 case 2 holds every node** — real, and "fails as singular rather than relaxing" understates it: it fails for circuits whose state is perfectly consistent, and it loses state that is not a potential. → M1.
3. **`analysis(Tran) && analysis(Static)` is cryptic** — accept as is. It occurs twice (`CapIc`, `zi_nd`), both commented, and it is two orthogonal flags composed, which is the point of having them. A stdlib `fn` could name it at zero construct cost; not needed.

### Is the repetition gone?

Partly. The two rules v3 named are now stated once each: the `settle` commit (one bold paragraph in 15; the 13 table, 15 steps 1/4, 16.1, 16.2, 17.2, C.1 only point at it) and the first-commit rule (13 rule 6; 5.2, 16 step 2, 23.1, E.1, 22.5 point at it). `t_next` 5 → 3, "explicit lag" 4 → 2, `(0, 0)` 16 → 12. That is honest dedupe.
The byte count hides it and flatters it at once: about 1.4 KB of the saving is Appendix H's history rows, about 2.5 KB is real dedupe in the body, and about 3 KB of new semantics went in. Net body text is flat. What remains is not repetition of a rule but length: 147 KB, with `analysis(Static)` now explained in six places (11.1, 13 rule 7, 16, 16.2, 22.2 prose, 22.5 doc comment). Acceptable; one more pass could take 16's closing paragraph down to a pointer at 11.1.

---

## MAJOR

### M1 [HOLE] 16.4 case 2 into a `Tran` breaks three ways (16.4, 16, 16.3 PSS row)
```rust
module Buck(sw: Electrical, out_: Electrical, in gate: Bit) { l: f64; c: f64; signal il: … }
analog for Buck { v(sw, out_) +> ddt(l * i(sw, out_)); i(out_, gnd) +> ddt(c * v(out_)); }
let vdd = Vsrc(vin, gnd, 3.3);                 // any ideal source
let z = zi_nd(v(out_), 1u);                    // contains every(period, period): signal k: Bit<u64>
let w = MinPulse(gate);                        // contains since(): signal t: Bit<f64>
// host: tran(0, 1m); then a new tran "from the final state"   (16.3: one of the two ways PSS is built)
```
- **Every node held, every ideal source singular.** `vin` is held by a source to `gnd` and driven by `Vsrc` to `gnd`: two potential sources in a loop. 16 only says a hold that *contradicts* fails; one that agrees is the same rank-deficient matrix with the split of flows undetermined. So case 2 fails or is undefined for every circuit with a supply, not only for the inconsistent ones of weak point 2.
- **Flow states are lost.** "The final potentials" carry `c·v` but not `l·i`: the inductor is a 0 V potential branch between two held nodes at different potentials (contradiction), and if they agree its current is free. A buck converter or LC oscillator cannot be restarted from its state.
- **Time restarts under carriers that hold absolute times.** `every`'s `k` is committed at `(0, 0)` with its final value, so its `at(t0 + k·period)` is armed a whole previous run into the future: `zi_nd` stops sampling ("every scheduled clock arms itself again" is false for `every`). `since`'s `t` is committed known and positive, so `width = 0 − t < w` at `(0, 0)` and `MinPulse` reports a pulse that never happened; 22.5's "`since` is still `inf`" no longer holds.

Fix, shorter than what is there and one rule instead of two: **an analysis may be run at the last instant of a transient with everything kept.** If it is a `Tran`, that is case 1. If it has no time (`Dc`, `Ac`, `Noise`), it is one more instant of that transient at which time does not advance: `now()` stays, discrete state stays (nothing is re-committed, no `changed()`, no `init()`), the queue stays and never comes due, the final solution is where Newton starts, nothing is held. PSS is then continued transients only; a shooting update of the state is the simulator's procedure, not something a model can see. Delete "started from the last one's final state" in 16.3.

### M2 [HOLE] A sampled real value has no response in the periodic small-signal definition (11.1 second bullet, 16.3, 23.3 rule 4)
```rust
module SampleHold(vin: Electrical, vout: Electrical, in clk: Bit) { signal held: Bit<f64> = 0.0; }
digital for SampleHold { on clk.rise() { held <- v(vin); } }      // 12.1
analog for SampleHold { v(vout) +> held.known_or(0.0); }
```
11.1 now says "around a periodic trajectory a carrier's values still have no response, but the times of its events do (23.3), which gives every sampled system one with nothing written". Here the event time is `clk`'s and does not move with `vin`; what moves is the value committed. Under the stated rule `vout` has zero response to `vin`, where the truth is the zero-order-hold response. The same for `DeltaSigma1`'s sampled input, `zi_nd` in its sampled form, every `Bit<f64>` in the SAR DAC. 23.3 rule 4 and its last sentence ("a `Bit<f64>` sampled at that edge has derivative zero") have the same error for param gradients, where it predates this round: `∂held/∂p = ∂v(vin)/∂p` at the edge, plus `v'(vin)·dt*/dp` when the edge itself moves.

Fix, in 23.3 rule 4 only: *a commit of a real value carries the sensitivity of the expression committed, taken at the commit's instant; a value of any other type has none and acts only through event times.* Then 11.1's bullet reads "a real carrier carries the perturbation committed to it; event times move as in 23.3", and G.30's loop filter has a response exactly as far as its values are real, which is the truth about a quantizer.

---

## MINOR

- **m1 [INCONSISTENT] 16.2 and 11.1 were not updated for the periodic analysis.** 16.2's small-signal column has `now()` = 0, 16.3 says "the point's time". 11.1's table says `Tran`/`Ac` are constant during an analysis, 16.3 has one analysis that is `Tran` then `Ac`. `zi_nd` tests `analysis(Tran)`, so in the linearization it switches to its `delay` form: a good answer (`H(exp(jωT))`), but then the equations linearized are not the ones the trajectory executed, which the small-signal rule already allows ("under the sweep's own flags"); say in 16.3 that this is intended. One footnote under 16.2: "in a periodic analysis the last column is taken at each point: `now()` is its time".
- **m2 [HOLE] A hold that agrees with a potential contribution (16).** Structurally the same singular loop as one that contradicts. Either the contribution stands and the hold is not applied to that node, or it is always an error. M1's fix removes the mass case; the declared-`= value` case still needs the word.
- **m3 [HOLE] Three unsaid things about continuing (16.4 case 1).** A host `set` or drive made between the two runs is a commit at the resume instant (15 step 2), not an initial value. The old stop time stays a time point with its `settle`, so a run continued at `T` differs from one uninterrupted run by one landing: say it is allowed to. A run ended by a `finish` record: continuable or not.
- **m4 [INCONSISTENT] 15 step 3.4's parenthesis.** "to a value the solve had not already read (a `settle` commit, below)" attaches the example to the wrong side; write "(a `settle` commit is one it had, below)".
- **m5 [BLOAT] 13 rule 7 is a reading guide, not a rule.** "The start is said three ways" lists two stdlib names and a guard defined elsewhere. Move it under the rules as prose; rule 6 then stands alone and the list has seven rules, each a rule.
- **m6 [BLOAT] 16's last paragraph restates 11.1** (`analysis(Static)` "true through the whole of this instant, in `analog` and in reactions"). Point at it.

---

## Elegance score

| Axis | v1 | v2 | v3 | v4 | Justification (v4) |
| --- | --- | --- | --- | --- | --- |
| 1. Orthogonality | 6 | 8 | 8 | 8.5 | `starting()` gone, so "the start" has one guard; `log` is not a third reserved stream. Left: `i(p)` vs `inflow()`; node `= value` as hint and as hold, now stated as deliberate. |
| 2. One rule per concept | 6 | 7 | 8 | 8 | `Static` settled in one sentence, both `settle` corners folded into the one statement. New exceptions in 16.4 case 2: held only if `Tran`, core instance state fresh but stdlib state kept, time restarts under kept carriers (M1). |
| 3. Syntax economy | 6 | 7 | 7.5 | 7.5 | 65, unchanged; nothing added for continuation, periodic analyses or the opt-out of G.32, all of which are text over existing constructs. Still a long list. |
| 4. Stdlib-in-language | 7 | 8 | 8.5 | 9 | One name fewer, `transition` shorter and correct, `since` readable in `analog`; every stdlib module traced this round follows from the 14-row core with no clock assumption. |
| 5. Readability | 6 | 8 | 8 | 8 | The two repeated rules are said once; `analysis(Tran) && analysis(Static)` is commented; the body is no shorter and `analysis(Static)` is explained six times. |
| 6. Semantic closure | 6 | 7 | 8.5 | 8.5 | Continuation closed, held-node corners closed; the two additions that were meant to close PSS and periodic small-signal each open a hole (M1, M2). Net level. |
| **Mean** | **6.2** | **7.5** | **8.1** | **8.25** | |

Gate: 0 BLOCKER, 2 MAJOR (≤ 2), mean 8.25 (≥ 8.0). Met, at the limit on MAJORs. Both are one-paragraph text fixes that remove words (M1) or correct one rule (M2); neither adds a concept or a construct, and neither touches a model in the galleries except through a host analysis.

## Counts (spec_v4)

**Concepts: 11 claimed, 11 effective.**
**Constructs: 65 claimed (B.1 sums correctly), 65 honest.** Stdlib names: −1 (`starting`).

## Residual open items

- **16.4 case 2**: replace by "an analysis run at a transient's last instant keeps everything; without time it is one more instant where time does not advance". PSS by continuation only (M1).
- **23.3 rule 4**: a committed real value carries the sensitivity of what was committed; fix 11.1's bullet and 23.3's closing example with it (M2).
- **16.2 footnote** for the periodic analysis (`now()`, which flags), and say `zi_nd`'s switch to its `delay` form there is intended (m1).
- **Hold vs potential contribution on one node**: decide skip-the-hold or always-error (m2).
- **Continuation details**: host `set` between runs, the extra time point at the old stop time, continuing after `finish` (m3).
- **Periodic small-signal has no gallery model and no implementation** (C.2, G.6): add one traced example (`SampleHold` under PAC) when M2 is fixed, so the definition is exercised.
- **Length**: 147 KB with 11 concepts; 13 rule 7 and 16's last paragraph are pointers written as prose (m5, m6); 65 constructs remains the lowest-scoring axis and nothing in four rounds found a further cut that a model does not pay for.
- **Never compiled**: every gallery and stdlib module is hand-traced only; the first compiler pass should run `transition`, `zi_nd`, `inertial`, `Tran`/`Wire` relaxation and the 15.1 trace as its conformance set.

VERDICT: CONVERGED

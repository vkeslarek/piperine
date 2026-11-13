# Critique of spec_v3 (round 3)

Ranking: 0 BLOCKER, 1 MAJOR, 9 MINOR. Tags: [HOLE], [BLOAT], [INCONSISTENT].
Galleries and stdlib were traced by hand against the rules, not compiled (no compiler exists).
Elegance is scored on what the language adds to Rust; inherited Rust syntax is not penalized.

---

## Verification of changelog_v3 against critique_v2

| v2 item | Status | Note |
| --- | --- | --- |
| B1 `idt` with `ic` flat in AC | CLOSED | 11.1's rule is the one sentence asked for; the branch-kind row is gone. Traced: `idt(x, ic)` → `jω·y = x`; `idtmod` (`seat` low at the operating point) → `1/jω`; `CapIc` → a capacitor; `Pi`, `Water.absorbed` follow. |
| M1 stdlib seeds at `init()` | CLOSED, residue → m1, m2 | The builder's rebuttal of `started()` is right (a `settle` commit lands at the next time point). `Top` traced: reset commits `q` at `(0,1)`, `x.changed() && starting()` re-seats `y0`/`y1`, the first step has `dt = 0`: 0 V in `Dc` and in the transient's operating point, no ramp. `zi_nd` traced: seed due at the first time point, the arriving solve reads it, first tick consistent. `inertial` in a `Dc` sweep follows. `starting()` itself is a duplicate (m1). |
| M2 `settle` commit on the digital side | CLOSED, residue → m5 | Ordinary commit at `(t_next, 0)`; `PeakAlarm` alarms; step 3.4's exemption is the only special property; digital-only designs covered by "operating point and end are time points". |
| M3 node `= value` without `uic` | CLOSED | Held through a transient's operating point, released as a state. `DeltaSigma1`, `Pll`, `Bounded` start. `Dc` singular for a pure integrator is accepted (weak point 4). |
| m1 16.2 incomplete | CLOSED, residue → m3 | Two tables, jump column, `noise` does not read `psd` where it is 0, 16.3 maps the host analyses. The periodic row is phrased as a refusal, not a rule (m3). |
| m2 `inertial` / environment in definitions | CLOSED | 8.2 rule 5 states the reason, not a list of bans. |
| m3 relaxation | CLOSED | Bound is drives + 1; the two-`Bidir` claim is withdrawn; unknown leaves a `log` record. The record's stream is unsaid (m7). |
| m4 stdlib violations | CLOSED | No `matches!`; `last_crossing` known; `Short<N>`; `fn` `where` listed. |
| m5 four cuts | CLOSED | No `final`, `Ic`, `Nodeset`, `at` period anywhere in the text (grepped); `every` is four lines and `zi_nd` uses it. |
| m6, m7, m8 | CLOSED | Port-kind contrast stated at 8.1; template inside the module rule; recursion, `prev()` on resolved carriers, `Setup`/`MinPulse` said. |

Nothing REOPENED.

Builder's weak points:
1. **`starting()` assumes `t` starts at 0** — the weak point is real and avoidable: see m1 (it is `analysis(Static)` under another name) and M1 (the one place a non-zero start can come from).
2. **A bare `after` holds in `Dc`** — accept. It is VAMS (only time-0 digital events run in DC) and one rule. The language already has the opt-out without `inertial`: `q = e after if analysis(Tran) { tpd } else { 0.0 };` (8.2 rule 5 allows it; `after 0` applies where time does not advance, 16.2). Say that in G.32 instead of "the same four lines".
3. **PSS / distortion host-side only** — accept for PSS and distortion; see "PSS" below and m3.
4. **`Dc` of an integrating node is singular** — accept. It is the truth, the error names the node, and a continuation that invented a value would break 16 step 3.
5. **`transition` steps from an unknown start** — worse than stated: it also ignores `td`. See m4.

**PSS and periodic small-signal host-side only: acceptable for "100% mixed-signal"?** Yes for the language, with one sentence missing. The goal is models that can be written, and no model is unwritable here: shooting PSS is repeated transients, which the model sees as `Tran`; `analysis("pss")` is not in VAMS 2.4. Periodic small-signal is not a missing concept either: the small-signal rule read "at each point of the trajectory" plus 23.3 (event times move with a perturbation, saltation at located events) is exactly its definition. So "not offered" is a statement about a simulator written into a language spec. What is actually missing is under PSS, not above it: M1.

**Did 139 → 148 KB cost elegance?** Not in concepts (11 → 11) or constructs (66 → 65); the growth is 16.2/16.3 and the held-node paragraph, which buy closure. It did cost some readability by repetition: the `settle`-commit rule is stated in eight places (13 table, 15 steps 1 and 4, the bold paragraph, 16.1 rule 2, 16.2, 17.2 table, C.1) and "`now() == 0`" in seven. See m8.

---

## MAJOR

### M1 [HOLE] "Continue from the previous analysis's state" is one clause, and PSS, `starting()`, `init()`, held nodes and every scheduled clock depend on it (16.1 rule 5, 16.3, 22.1)
```rust
module ClockGen(out clk: Bit = false) { … }
digital for ClockGen { on clk.changed() { clk <- !clk after period / 2.0 + j; } }   // D.2
// host: tran(0, 1u); then "continue" tran for another 1u   (16.3: this is how PSS is built)
```
Nothing says what a continued analysis is. Each reading breaks something:
- Pending delayed commits: 16.1 rule 3 drops them "when the analysis ends" for analyses without time; for a transient nothing is said. If dropped, `ClockGen`, `Pfd`'s reset, every `transition` mid-ramp and every `every` are dead in the continuation: no `changed()` ever re-arms them.
- Does `now()` restart at 0? If yes, `starting()` is true again, every `at(t)` with an absolute time is in the future again (`SweptResistor`, `Pwl` replay; `since` goes negative). If no, there is a transient with no operating point and `starting()` never true.
- Is there an operating point? If yes, nodes with `= value` are held again (16) and the continued state of `DeltaSigma1.x` and `Pll.vc` is overwritten, which makes host-built PSS impossible for exactly the circuits that need it. Does `init()` fire (16.1 rule 2 answers for sweep points only)?

Fix, one paragraph, no construct: *a continued transient is the same analysis with a later stop time: time, the event queue, discrete state and instance state are kept; there is no operating point, nothing is committed or held, `init()` does not fire. Any other analysis started after a transient may take that transient's final state as its initial values (commits at its `(0, 0)`, node values as held values), and the queue is dropped.* Then 16.3's PSS row is true, and weak point 1 has no remaining source.

---

## MINOR

- **m1 [BLOAT] `starting()` is `analysis(Static)`.** In a reaction "the solve is the one whose solution the reaction reads" (11.1), so `analysis(Static)` is true through every round of the operating point and at every `Dc` point, and false after any step or jump: the same set as `now() <= 0.0`. In `analog` the two differ only inside a sweep, where `transition` gives the same answer either way (traced: `d == 0`, so `y = b = y1 = x`). Delete `starting()`; write `analysis(Static)` in `transition`, `inertial`, `zi_nd`. This removes a stdlib name, the time-origin assumption (weak point 1), and the "`now() == 0`" explanations in 13 rule 6, 16, 16.1, 22.2, 22.5. If the name is kept for reading, define it as `analysis(Static)`, not by the clock.
- **m2 [INCONSISTENT] `analysis(a)` has two regimes by variant.** Four variants are constant during an analysis and legal everywhere; `Static` belongs to the solve and is barred from definitions. 11.1 needs a column for it, 14 two rows, 8.2 rule 5 the phrase "`analysis` of an analysis kind". Either accept it and say once "`Static` is the one variant that names a solve" in 11.1 (cheapest), or move it out of the enum as an environment `fn`. Do not do both this and keep `starting()`.
- **m3 [HOLE] Periodic small-signal is refused, not defined (16.3, G.30).** Replace "not offered" by the rule: *a periodic small-signal analysis applies the small-signal rule at each point of a periodic transient solution; discrete state is the trajectory's, and a located event's time moves as in 23.3; the model sees `Ac` or `Noise`.* Whether a host implements it is C.2's business. G.30 then closes: a digital loop filter between an ADC and a DAC has a response under that analysis without being rewritten with `delay`.
- **m4 [HOLE] `transition` with an input that starts unknown ignores `td` (22.2).** `let x0 = x.known_or(0.0)` is read in every branch through `a`/`b`. With `y1` unknown (seeded from an unknown `x`), `b = x0` tracks `x` live: when `x` first becomes known at `t1` the output steps at `t1`, not at `t1 + td`, with no ramp; at `t1 + td` the edge sees `to == y` and does nothing. `DacRamp` with an unknown reset code does this. Use `x0` only under the `Static` test; elsewhere `y0.known_or(0.0)`, `y1.known_or(0.0)`. Then the first known value arrives after `td` with a ramp from 0.0, and D.4's "the output holds" is true.
- **m5 [INCONSISTENT] Two corners of the `settle` commit (15).** (a) Step 3.4 exempts a change "by a `settle` commit"; a definition recomputed from that carrier (`over = peak > lim`, read by `analog`) changes "by a definition". Say "by a `settle` commit and the definitions that follow from it"; the arriving solve must read those too. (b) A delayed commit from another statement to the same level, due at the same `(t_next, 0)` and made later, wins by 12.1 rule 2; the arriving solve read a value the carrier never held. Say that the arriving solve reads what `(t_next, 0)` will apply, whoever wins.
- **m6 [INCONSISTENT] Node `= value` is a hint in `Dc`/`Ac` and a constraint in `Tran`.** Accepted as SPICE's `.ic` (v2 asked for it), but 6's comment and 16 should say plainly that the operating point of a transient and a `Dc` of the same circuit differ on purpose for a held node that has a DC path, and that a model wanting only a hint leaves it to the host's nodeset. Also state the conflict case: a held node that is also the target of a potential contribution is an error naming both.
- **m7 [INCONSISTENT] Small leftovers.** The `log` stream: 8.2 rule 3 and 23.1 rule 3 have the simulator write it, 19 rule 2 says only `sim` and `checks` are reserved and global, and stdlib `log()` writes an instance-relative `log`. Pick: simulator records go to the instance-relative `log` of the loop's first drive, or `log` is a third reserved name (then 66). Section 3's table still says a `fn` that "reads the environment" is not callable in definitions; 14 and 8.2 rule 5 allow the constant part. 11 rule 3 lists what may select a branch's kind as "params, discrete carriers and `analysis`"; `now()` and `freq()` are also constant during a solve: say "anything constant during a solve".
- **m8 [BLOAT] Repetition.** State the `settle` commit once (the bold paragraph of 15) and reference it; 13 rule 6 is four rules in one paragraph (`changed()` at `(0,0)`, `init()`, `starting()`, `above`): split or cut after m1. Expect about 3 KB back with no content lost.
- **m9 [HOLE] Unsaid, small.** What a `settle` commit made at the stop time does (dropped; `Dut`'s `ipk <- pk` at the last point is lost, harmless because `target` already reported `pk`: say so). `since` returns `now() - t` on a bare carrier, so it cannot be read in `analog` (3.3 rule 5) although `transition`-like code wants it: `t.known_or(-inf)`. The release of a held algebraic node (no `ddt` on it) moves it between `t = 0` and the first step with no jump and no error estimate: say the first step is bounded as after a jump (15.2 rule 6).

---

## Elegance score

| Axis | v1 | v2 | v3 | Justification (v3) |
| --- | --- | --- | --- | --- |
| 1. Orthogonality | 6 | 8 | 8 | `at` period gone, port-kind contrast stated, template inside module; new overlap: `starting()` vs `analysis(Static)` (m1); left: `i(p)` vs `inflow()`, node `= value` as hint and as hold (m6). |
| 2. One rule per concept | 6 | 7 | 8 | Small-signal is one sentence and the branch-kind row follows from it; a `settle` commit is an ordinary commit; relaxation bound comes from the loop. Exceptions left: `analysis` regimes by variant (m2), the two `settle` corners (m5). |
| 3. Syntax economy | 6 | 7 | 7.5 | 65, with `final`, `uic`, eight `Analysis` variants and `at`'s period gone and nothing added; every remaining construct has a model behind it; 65 is still a long list for a newcomer. |
| 4. Stdlib-in-language | 7 | 8 | 8.5 | `init`, `every`, `since`, `idt`, `idtmod`, `slew`, `transition`, `inertial`, `zi_nd`, nets, converters, monitors, checks all written out over a 14-row core, and they trace; one bug (m4), one redundant function (m1). |
| 5. Readability | 6 | 8 | 8 | Models read like the circuit; `transition` got shorter; `analysis(Tran) && analysis(Static)` for "the transient's operating point" is cryptic; the spec repeats itself (m8). |
| 6. Semantic closure | 6 | 7 | 8.5 | 16.2's two tables and 16.3 close every primitive in every solve and at every instant; open: continuation between analyses (M1), periodic small-signal as a refusal (m3), the `log` stream (m7). |
| **Mean** | **6.2** | **7.5** | **8.1** | |

Gate: 0 BLOCKER, 1 MAJOR (≤ 2), mean 8.1 (≥ 8.0). Met.

Changes that would raise the lowest axes most, if a further round is run:
1. **Syntax economy / orthogonality:** delete `starting()` in favour of `analysis(Static)` (m1) and settle the `Static` wart in one sentence (m2).
2. **Closure:** the continuation paragraph (M1) and the periodic small-signal sentence (m3); both are text, neither adds a construct.
3. **Readability:** state the `settle` commit and the time-0 rules once each (m8); fix `transition`'s unknown start (m4).

## Counts (spec_v3)

**Concepts: 11 claimed, 11 effective.** Template is decided by the module rule as now written.

**Constructs: 65 claimed (B.1, sums correctly), 65 honest; 66 if `log` becomes a reserved stream (m7).** Reachable: 65 with `log` instance-relative; the stdlib loses one name (`starting`) with m1.

VERDICT: CONVERGED

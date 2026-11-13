# Critique of spec_v1 (round 1)

Ranking: 0 BLOCKER, 7 MAJOR, 11 MINOR. Tags: [HOLE], [BLOAT], [INCONSISTENT].
Galleries were checked by hand against the rules, not compiled (no compiler exists).

---

## Verification of changelog_v1 against critique_v0

| v0 item | Status | Note |
| --- | --- | --- |
| B1 multi-driver / strengths / inout | CLOSED, residue → M4, M5 | Trees of `Tran` work (checked F.4, chains, pass-gate mux). Any *cycle* through `others()` is still an elaboration error, and `=` on two `Wire`s is ambiguous. |
| B2 jump solve | CLOSED | Rule is single and F.5 is right (0.25 V). Differentiator→integrator through an impulse also comes out right. Residue: settle (M1), restart cost (m4). |
| B3 unknown values | CLOSED | One flag, two operator classes, one merge rule. "No exceptions" is false in one place (m2). |
| M1 idt / idtmod / Nullor | CLOSED | `idt` hold in `Static`, state on nature `S`, `idtmod` re-seat traced through three microsteps: correct. `Servo` row correct. |
| M2 collision / transport | CLOSED, new trap → m9 | Last-write-wins is deterministic. The removal rule now cancels across unrelated statements. |
| M3 time-0 consistency | **REOPENED** → M2 | The rule is coherent only for `= value`; the host's drive at t = 0 *is* a change, and E.1 was not updated. |
| M4 analyses without time | CLOSED, residue → M3 | 16.1 is clear. Delayed *definitions* contradict it. |
| M5 two kinds of callable | CLOSED | No `fn` in the spec calls a module (checked `limexp`, `white_noise`, `above`, `vt`, `table_model`, `handshake`). |
| M6 value ports | CLOSED as a rule, not as a count → M7 | Call-by-name is now stated. It is a concept, not a "cover". |
| M7 small-signal of sampled state | CLOSED | `zi_nd` via `delay` gives `H(e^{jωT})`. Residue in m1. |
| M8 switch-branch selector | CLOSED, residue → m5 | |
| M9 17.2 rule 2 vs table | CLOSED for scheduled instants; the `settle` row is wrong → M1 | |
| M10 `above`/`absdelta`/`transition` | CLOSED | `transition` traced: ramp start, mid-ramp retarget, far corner, tie order all correct. |
| M11 views | CLOSED | |
| m1–m9 | CLOSED | m4: new grammar ambiguities (m6). m6: one structural case missing (m5). |

Builder's own weak points:
1. **Ring of `Tran`s** — not just rings: see M4. Not acceptable as an open question.
2. **`known_or` on own state** — accept as is. The rule without a static-proof exception is the right trade; no action. (Cost counted: about 25 sites in the model code.)
3. **Unknown index merges into every word** — accept. It is exact and consistent with 5.1 rule 3: a never-written word is unknown and `merge(Unknown, w) = Unknown`, so only materialized or initialized words are touched. Say that sentence in E.5.
4. **Jump as a limit, implementation `h` = one tick** — accept; error is `O(h/τ)`.
5. **`idt` without `ic` outside feedback** — accept (VAMS 4.5.4).

---

## MAJOR

### M1 [INCONSISTENT][HOLE] `on settle` committing something `analog` reads is a jump at every time point (9 case C, 15 steps 2–4, 17.2 table)
```rust
digital for LaggedResistor { on settle { r <- r0 * (1.0 + k * v(ctl)); } }
analog  for LaggedResistor { i(p, n) +> v(p, n) / r.known_or(r0); }
```
15: settle commits are "held until the next time point", collected at step 2 *after* the left solve (step 1), applied in the microstep loop; step 3.4 then says "a carrier read by `analog` changed: solve the jump". 15.2 rule 6: the integrator restarts. So this model does, per time point: solve, two backward-Euler jump solves, restart at a tiny step, with the method degraded to first order forever. The 17.2 table says `on settle` → jump "no", and row 2 says a discrete change read by `analog` → jump "yes". Both rows apply to the same commit. 9.C claims "no time point of its own" and implies no cost.
Needed: one sentence on *where* a settle commit takes effect. Either it applies before step 1 of the next time point (the carrier is constant over the step `(t_k, t_{k+1}]`, there is no left/right pair, no jump, no restart — an explicit lag), or it is an ordinary jump and case C must say so and stop being offered as the cheap option. Any model that uses `settle` for a running estimate fed back to `analog` (adaptive bias, AGC, thermal lag) depends on the answer.

### M2 [HOLE] Time 0 depends on who supplies the value (13 rule 6, 16 step 2, 23.1 rule 4) — v0 M3 reopened
"Initial values are not changes" covers `= value`. But a top-level `in` port has no `= value`: the host is its driver, and 23.1 rule 4 says a host drive "enters the instant at step 2 of section 15, like any other commit". So:
```rust
module Dff<T: Repr>(in clk: Bit, in rst_n: Bit, in d: Bit<T>, out q: Bit<T>) { reset: T; }
digital<T> for Dff<T> { on clk.rise() || rst_n.fall() { if !rst_n { q <- reset; } else { q <- d; } } }

module Por(out rst_n: Bit = false) {}                         // reset generator inside the design
digital for Por { on at(100n) { rst_n <- true; } }
```
- `rst_n` driven low by the host at t = 0: unknown → L is a change, `fall()` fires (22.1 counts `None → Some(false)`), `q` resets.
- `rst_n` low from `Por`'s `= false`: nothing fires. If the clock is gated during reset, `q` is unknown forever. Same netlist, different result, decided by whether the reset source is inside or outside the top module.
- A host clock that starts high fires `rise()` at (0,0); a `ClockGen` with `= true` does not. The † rebuttal ("`rise()` on a clock that starts high would need exceptions") describes exactly what the host path already does.
- 16 never says whether host drives at t = 0 are part of step 2 ("Initialize") or are commits in the step-4 loop, nor what an `in` port holds at a DC sweep point.
- E.1 has no `init`, against 13 rule 6's own instruction; every asynchronous set/reset is level-sensitive.

Pick one and apply it to both sources. Option A: every initial value, model or host, is a commit from unknown at (0,0) (Verilog's behavior; `init` then becomes derivable and E.2's `init ||` goes away). Option B: host drives at t = 0 are initial values too (step 2), nothing but `init` fires, and E.1 and every async-reset model gets `init ||`. Either is one rule; today there are two.

### M3 [INCONSISTENT][HOLE] A delayed definition has zero delay at the operating point; a delayed commit is dropped (8.2 rule 4 vs 16.1 rule 3)
```rust
module Ring3(out y: Bit = false) {
    tpd: f64 = 10p;
    signal a: Bit = true;  signal b: Bit = false;
    a = !y after tpd;  b = !a after tpd;  y = !b after tpd;
}
```
Elaboration accepts it (each drive has a delay, 8.2 rule 3). 8.2 rule 4: "At an operating point `y` has the value of `expr`" — so at the OP this is `y = !!!y`, a zero-delay odd loop with no fixed point: step 4 of 16 runs out of rounds and the analysis is "an error naming the carriers". A ring oscillator of gates cannot start a transient. The same oscillator written with reactions (`on y.changed() { a <- !y after tpd; }`) starts fine, because 16.1 rule 3 says a delayed commit "never comes due".
`SrLatch` (E.2) has the same problem in the other direction: at the OP it is the zero-delay cross-coupled pair the text calls "a combinational loop, an elaboration error"; which of its two fixed points holds for `s = r = 0` is not stated (and for `s = r = 1 → 0` in a DC sweep there is none that is reachable deterministically).
One rule for both forms of `after`: at an operating point a delayed drive keeps its current value unless `expr` has settled to something else along a dependency path with no cycle; a cycle through `after` holds its initial values. Or simpler: `after` is "next round" in step 4 and non-convergence of *delayed* drives leaves them at their initial values instead of failing.

### M4 [HOLE] Any cycle through `others()` is an elaboration error (5.2, 8.2 rule 3, 22.3, 22.4, G.27)
Not only rings:
```rust
let n = Tran(a, y, s);       // CMOS transmission gate written the switch-level way:
let p = Tran(a, y, sn);      // tranif1 and tranif0 in parallel
```
`n`'s drive of `a` reads `y.others()` ⊇ `p`'s drive of `y` = `a.others()` ⊇ `n`'s drive of `a`. Two-element cycle: rejected. Also rejected: a redundant bidirectional bus ring, two `Bidir` pads on one `Wire` (each drive reads the other's), any imported netlist with `tran` in a mesh. `Tran` trees are the only legal topology and the elaborator, not the author, finds out.
G.27's third option needs no syntax and should be the rule: a dependency cycle made only of drives of resolved carriers is evaluated by iteration from "no drive" (`resolve(&[])`) each microstep, to a fixed point; a bounded count; no fixed point → the carriers on the cycle are `Unknown`. Starting from empty every time means nothing latches. Requirement on `resolve` becomes "commutative, associative, idempotent" for natures used in a cycle (true of `Logic::wired` and `tri`; false of `sum`, which then keeps the error).

### M5 [INCONSISTENT] `y = x;` between two `Wire`s is a short, between two `Bit`s is a buffer (0 "Who writes what", 8.2)
"`=` between two nets (… two carriers of one resolved nature) makes them one net. The right side is a bare carrier." versus "`=` defining a discrete carrier makes this module's drive of it a function".
```rust
module Buf(a: Wire, y: Wire) { y = a; }            // author means Verilog `assign y = a;`
```
This merges `a` and `y`: drivers of `y` now fight drivers of `a`, silently. `y = a.others();`, `y = (a);` or `y = Logic::strong(a.v);` would be a one-way drive. Bareness of the right-hand side selects between a symmetric and a directed meaning, for the most common statement in an imported netlist. And on `Bit` the same text `clk2 = clk;` is directed (8.2 `Shift2`).
Delete net-`=` for resolved carriers: nets are joined by passing one carrier to two ports (structure already does this), and by a stdlib `Tran(a, b, true)` once M4 is fixed. For nodes, 11 rule 3 already merges them (`v(a, b) +> 0.0` under a param selector). That removes one of the two meanings of `=` and one row of the "Who writes what" table. If `n[0] = a;` in generation is wanted, keep it for nodes only, where no directed reading exists.

### M6 [BLOAT][INCONSISTENT] The stdlib macros cannot be written in the language (19, 20, 22.1, 22.6, B.1)
Fifteen macros appear: `println!` `eprintln!` `info!` `warning!` `error!` `assert!` `finish!` `fatal!` `require!` `target!` `cover!` `logic!` `matches_bits!` `include_table!` `include_mem!`. A.1 says the lexer and grammar are the language's own; no macro-definition form is in B.1 or B.3. `require!(head_hi, v(p, n) <= hi)` "also reports its margin", which needs the expression split at the operator: not a function. `include_table!` returns an array whose length comes from a file.
Either `macro_rules!` (and `include_bytes!`, `const fn` parsing) is inherited from Rust — say so in A.1 and count one construct — or these fifteen are core and B.1 is 80, not 65. "Everything else is the standard library written in the language" is unproven until one of them is shown.
Shrink regardless: `require!`/`target!`/`cover!` differ only by a tag field (`check!(kind, name, cond)`); `info!`/`warning!`/`error!`/`eprintln!` likewise; `fatal!` = `error!` + `finish!`. Five macros cover the fifteen.

### M7 [BLOAT] The concept table is 15 concepts with 10 rows (section 0)
A "Covers" entry is honest only if the row's one rule decides it. These have rules of their own that the row's rule does not imply:

| Hidden under | What | Its own rule |
| --- | --- | --- |
| 3 carrier | **branch** | "runs between two nodes; contributed to and probed"; not declared with a nature, cannot be connected, has a *kind* per solve (11 rules 2–4). Fails the row's rule ("a signal or port of a nature … can be connected"). |
| 2/3 | **drive** | "each module holds one drive; `=` xor commits; `others()`" (5.2). Neither the nature's rule nor the carrier's. |
| 5/6 | **expression read in place** | body `let`, value input and result share one call-by-name rule (8.2, 8.3 rules 1–2) with three evaluation regimes (iterate / instant / located). It is neither value, carrier nor instance. The row for 6 states it as a second sentence; the row for 5 lists `let` as an item. |
| 4/6 | **template** | a module as a value: struct-update, `Fn(P)` bound, callable, returned by `fn` (8.4). "Templates as params" under 4 and "templates" under 6: listed twice, ruled by neither. |
| nowhere | **instant** | `(t, n)`, the microstep loop, the jump, the operating-point fixed point (Part V). The largest body of semantics in the spec has no row. |

Also: row 5's "rule" is a list of items, row 6's is two rules, row 10 mixes host and `emit!`. `let` has four roles (by-name expression, instance placement, named branch, eager local in a reaction with `mut`).
Honest ways down: (a) merge *template* into *param* by saying "a module with its params bound is a value of the module's type" and delete struct-update on templates (binning is `fn(l, w) -> Bsim4` already); (b) make *expression read in place* the stated rule of `let` and define value input and result as "a `let` bound across the instance boundary" (one rule, row 6 loses its second sentence); (c) add *instant* as a row and drop "host" as a language concept (it is outside the language by Part VIII's own argument; `emit!` goes under `digital`/`analog`). That gives 12 rows, each with one rule.

---

## MINOR

- **m1 [INCONSISTENT] Gallery violations found by hand.**
  - D.4 `DacRamp`: `transition(vref * (code as f64) / 256.0, …)` inside `analog` reads `code` without `known_or` (3.3 rule 5 as written), and creates a definition inside `analog` (B.2 allows `=` in the body only). Exempt `in`-port arguments explicitly or move it to a body `let` as `D2A` does.
  - E.1 `Dff`: no `init` (M2).
  - D.1 `Pwl`: "the host may `set` `points` during a run". 13 rule 4: re-armed `at`s whose time is past "fire at the current instant" → every `seg <- k` fires in one microstep, last wins (12.1 rule 2), `seg = N − 1`. Delete the fire-if-past clause: a re-armed time already past does not fire (consistent with "initial values are not changes").
  - 22.2 `zi_nd`: comment says `xs[k] = x[n−k]`, code uses `xs[k−1]`; `analysis(Tran)` is true in the transient's own operating point (11.1), so the transient starts from `out = 0` while a `Dc` gives `H(1)·x`; no syntax shown for params (`num`, `den`) of an instance placed inside an expression.
  - D.3 `Vco`: `clk = false` at t = 0 with `ph = 0`, inside the high half; first edge is 3/4 cycle late. Use `above`.
- **m2 [INCONSISTENT] 3.3 rule 3 has an exception and two gaps.** "A loop whose exit is unknown stops there, and everything its body could write becomes unknown" is a second rule (over-approximation, not merge), next to the sentence "Rule 3 has no exceptions". Unspecified: a commit `after d` reached under unknown control (does a "maybe" commit remove pending ones, 12.1 rule 4?), and `after d` with `d` unknown.
- **m3 [INCONSISTENT] `sum` is not associative in floating point**, so `SumNet` is order-dependent and "the result is a function of the source alone" (15) fails for the stdlib's own nature. Pass drives in a fixed order (instance path) and drop the commutativity requirement from 4 rule 2, except where M4 needs it.
- **m4 [HOLE] Unconditional restart after every jump (15.2 rule 6, C.2).** `transition` makes two analog-read carrier changes per edge (start corner: value continuous, only the slope changes), `idtmod` two per wrap. Each restarts the integrator at 10⁻³ of the last step, first order. A 1 GHz `Vco` restarts every cycle; a `D2A` on a bus restarts on every bit. State the guarantee instead: after a jump, history is dropped only for states whose value or derivative differs between the left value and the second pass. Without it, moving `transition`/`idtmod` to the stdlib (v0 M10, M1) is a regression in accuracy, not just speed.
- **m5 [INCONSISTENT] Structural params.** 11 rule 3: a param-only branch selector "merges its two nodes at elaboration" (`rs: Option<f64>` in `Diode`). 8.1 rule 2's list of what makes a param structural omits it; a host `set` of `rs` from `None` to `Some` mid-run changes topology.
- **m6 [HOLE] Grammar.** `on cross(e) { … }`, `if above(x) { … }`: `instance ::= path '(' … ')' ( '{' IDENT ':' expr … '}' )?` makes the block a param list; Rust's "no struct literal in a condition head" restriction is not stated. `fn cast … where A::WIDTH == B::WIDTH`: `fn_decl` has no `where`. `place` in `commit` is undefined: is `q[7..4] <- x` or `st.n <- 3` on a `Bit<u8>`/`Bit<S>` legal? 12.1 rule 2 speaks of per-level commits, but only whole-carrier and array-element commits are shown; two reactions writing different fields of one word in one microstep currently clobber each other (read-modify-write + last write wins), where Verilog part-select NBAs compose.
- **m7 [HOLE] `emit!` in `analog` runs on rejected iterations (19 rule 3); `finish!` and `require!` are `emit!`.** A `finish!()` or a failing `require!` reached on an unconverged iterate ends or fails the run depending on the Newton path, against "every outcome is defined by the source". Records from `analog` are debug-only: restrict the `sim` and `checks` streams to reactions.
- **m8 [BLOAT] Four more deletions.** `slew` is the `Bounded` pattern of 11.2 (track / rate-limited modes selected by a `Bit`, crossings of `ddt(x) − rise` and of `x − y`), which the spec calls exact: stdlib. `since()` is a three-line module with a result (`on init || s.changed() { t <- now(); }`), and removing it halves per-carrier history (5.3, m9 of v0). `#[nodeset]` is a hint, not an equation (18's own words): host, like `#[mc]` was. `#[allow(nonsmooth)]` exists only to silence a lint G.3 calls too coarse: delete both. −4 constructs.
- **m9 [HOLE] Transport removal crosses statements (12.1 rule 4).** F.2 `Pfd`: both high at `t`, reset scheduled for `t + 50p`; `fref` rises again at `t + 20p`; `up <- true` is "a commit made for time T" and removes the pending `up <- false`. `dn` resets, `up` stays high for a whole extra cycle; silent, wrong loop dynamics. Also `p <- false after w; p <- true;` never ends the pulse, while the reverse order works, against E.3's "in any statement order". Scope the removal to pending commits *made by the same statement* (one transport queue per `<-`/`=` site; sites then combine by last-write-wins). `TwoPhase` (one site) keeps working.
- **m10 [HOLE] Noise through operators.** 11.1 defines correlation as "same instance / different instance". Induced gate noise needs a 90° partial correlation: is `ddt(n)` (or `delay`, `laplace_*`) of a `noise` result its small-signal image `jω·n`? One sentence in 11.1.
- **m11 [HOLE] Inertial delay exists only for `Bit<T>` in/out (22.5).** Verilog gate delays are inertial by default and to-Z delays apply to tristate drivers; a drive of a `Wire` has transport `after` only. `Inertial` should be generic over the carrier's nature or shown for `Wire`.

---

## Elegance score

| Axis | v0 | v1 | Justification (v1) |
| --- | --- | --- | --- |
| 1. Orthogonality | 4 | 6 | Bus modules, views, `interface`, three callables are gone; still two meanings of `=` chosen by bareness (M5), `let` with four roles, template listed under two concepts (M7). |
| 2. One rule per concept | 4 | 6 | Unknowns, drives, jump and callables each have one rule now; exceptions remain at time 0 (model vs host, M2), `after` at the operating point (M3), settle commits (M1), unknown loops (m2). |
| 3. Syntax economy | 5 | 6 | 65 claimed against 80 before, but 72–87 honest (M6); `slew`, `since`, `#[nodeset]`, the `nonsmooth` lint do not pull their weight (m8). |
| 4. Stdlib-in-language | 5 | 7 | `Bit`, `Wire`, `idt`, `idtmod`, `transition`, `above`, `zi_nd` are written out and trace correctly; the fifteen macros have no definition mechanism (M6) and `others()` cycles need the elaborator (M4). |
| 5. Readability | 7 | 6 | Models still read like the circuit, but `known_or` at about 25 sites, `init \|\|` on level-sensitive logic, and transport removal across statements (m9) make behavior harder to predict than in v0. |
| 6. Semantic closure | 3 | 6 | Jump, analyses without time, small-signal of sampled state and X are now defined; open: settle→analog, time-0 host drives, delayed definitions at the OP, noise through operators (M1–M3, m10). |
| **Mean** | **4.7** | **6.2** | |

## Counts (spec_v1)

**Concepts: 10 claimed, 15 effective** (the ten rows + branch, drive, expression-read-in-place, template, instant; M7). Reachable: 12 honest rows.

**Constructs: 65 claimed (B.1). 72 with forms used but unlisted**: `impl` on a nature/bundle with a handle `self`, derived nature (`: Base`), `Option<N>` ports with elaboration `if let`, `pub` on body items (`pub let`, `pub signal`), const-expression `where` on modules and `fn`s, the reserved stream `sim`, the `collision` lint. **87 if the fifteen macros are core** (M6); 73 if `macro_rules!` is declared inherited.

Reachable next round: −`slew`, −`since`, −`#[nodeset]`, −`#[allow(nonsmooth)]` (m8), −net-`=` on resolved carriers (M5, a rule, not a token), macros 15 → 5 written with an inherited `macro_rules!` (M6), template struct-update dropped (M7a): **about 67 honest constructs, 12 concepts.**

VERDICT: CONTINUE

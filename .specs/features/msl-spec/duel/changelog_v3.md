# Changelog v2 → v3 (builder, round 3)

Answers `critique_v2.md`. B1 and the three MAJORs are fixed; all eight MINORs are fixed. Two are fixed differently from the critic's proposal (marked †, with the reason). Nothing is rebutted.

## Items

| Item | Action | What changed | Where in `spec_v3.md` |
| --- | --- | --- | --- |
| **B1** `idt` with `ic` flat in AC | fixed (critic's sentence) | **The small-signal rule:** the sweep linearizes the equations `analog` executes under the sweep's own flags (`analysis(Static)` false, `ddt → jω`), at the operating point's solution and discrete state. The "branch's kind" row of 16.2 is deleted: the kind follows from the rule. `idt(x, ic)` is `1/jω`; so are `idtmod`, `Pi`, `Water`'s `absorbed`. | 11.1, 8.3 (`idt` paragraph), 16.2 |
| **M1** stdlib seeds at `init()`, not at the settled operating point † | fixed | † Not a `started()` module on the first `settle`: a `settle` commit lands at the next *time point*, which in a digital-only design is the end of the run, so `inertial` would never start. The whole operating point, every round, has `now() == 0`, and so does every analysis without time; the stdlib `fn starting() -> bool { now() <= 0.0 }` has no state at all. `transition` rests on its input while `starting()` (the critic's `Top` now gives 0 V in `Dc` and in the transient's operating point, no opening ramp) and lost its `!analysis(Tran)` and `start` terms; `zi_nd` seeds on `settle && starting()`; `inertial` follows its input while `starting()`. `analysis(a)` inside a reaction is defined: the solve whose solution the reaction reads. | 22.1, 22.2, 22.5, 13 rule 6, 16, 11.1 |
| **M2** `settle` commit on the digital side | fixed (critic's first option) | A `settle` commit is an ordinary commit due at `(t_next, 0)`: it fires `changed()` there and its reactions run in that instant's loop (`PeakAlarm` alarms). Its only special property is that the arriving solve already reads it, hence no jump. Digital instants in between read the old value; a rejected step moves the commit with `t_next`. The separate "Arrive" step is gone. The operating point, the stop time and a `finish` instant are always time points, so `settle` is defined in a design with nothing analog. Sweep points: due as the next point begins. | 15 steps 1–5 and the two paragraphs after, 13 table, 16.1 rule 2, 17.2 rule 2 and table, C.1 |
| **M3** node `= value` without `uic` | fixed (SPICE's rule), `uic` deleted | A node's `= value` is where Newton starts; through a transient's operating point it is also held, and released as the first step begins (not a jump: `ddt` arguments are the states). `DeltaSigma1` and `Pll` start; their `Dc` fails as singular, which is the truth. No `uic`, no `Analysis::Ic`. A floating capacitor's initial voltage is the switch-branch pattern on `analysis(Tran) && analysis(Static)` (`CapIc`), and by B1's rule it is a capacitor in a sweep. | 16 (rule and `CapIc`), 6, D.5, C.1, C.2 |
| m1 16.2 incomplete | fixed | Two tables. Solves: four columns (equilibrium, transient step, **jump**, sweep), with rows for probes and `inflow`, `now()`, `freq()`, `analysis(Static)`, the constant environment, `delay` for `t < td`; `noise` does not read its `psd` where it is 0. Instants: zero-delay commits, `cross`, `at`/`after`, `settle`. New 16.3 maps every host analysis (`Tf`, `Pz`, `Sp`, sensitivities, distortion, PSS) onto the five kept variants; periodic small-signal is stated as not offered. `Analysis` is closed: `Dc`, `Tran`, `Ac`, `Noise`, `Static`. | 16.2, 16.3, 11.1 |
| m2 `inertial` unknown without time; environment in definitions | fixed | `inertial` passes through while `starting()`. A definition may read what is constant during an analysis (`temperature()`, `simparam`, `analysis` of a kind); only `now()`, `freq()`, `analysis(Static)` and probes are barred, and the reason is the definition's own rule (recomputed only when a carrier changes). | 22.5, 8.2 rule 5, 14, B.2, G.32 |
| m3 relaxation | fixed | (a) A loop gets as many passes as it has drives, plus one; the constant 64 is gone. (b) The two-`Bidir` claim is withdrawn: a drive that drives because others do not has no fixed point; said in 22.4. (c) **A loop that does not settle goes unknown and the simulator writes a `log` record naming its drives.** G.31 closed. | 8.2 rule 3, 22.3, 22.4, C.2 |
| m4 stdlib violations | fixed | `rise`/`fall` are two comparisons on `level`, no `matches!`; `last_crossing` returns `t.known_or(-1.0)`; `Short<N: Nature>` over `potential`; `where` over const generics on a `fn` is in the forms row of B.1. | 22.1, 22.2, 11, B.1 |
| m5 four cuts | deleted | `final` (the reduction reports from `on settle`; the host keeps a target's last record), `Analysis::Nodeset`, `Analysis::Ic`, `at`'s period (stdlib `every(t0, period)`, four lines; 13 rule 4 lost a clause). | 13, 14, 19, 22.1, 22.6, B.1 |
| m6 two expression-passing ports | fixed | One sentence at the port table (discrete `in` expression vs continuous value input, with the `transition(v(a))` example) and the contrast named in 8.3 rule 1. | 8.1, 8.3 |
| m7 concept-table riders | fixed | Template moved into the module rule ("with its params bound it is a value, and calling that value places an instance"); core operators listed under module, not `let`; `let mut` said to be a value-level local. | 0, 8.2 |
| m8 unsaid things | fixed | Recursive generation allowed, unrolled, must end; `prev()` on a resolved carrier is the previous settled value; `Setup`/`MinPulse` measure from the `(0, 0)` commit. | 8.2, 5.3, 22.5 |
| weak point 1 (start-high clock) | fixed | Said next to `Counter`, with the one-guard opt-out. | 12.1 |
| net `=` riders | fixed | `Short` generic; `Tran(a, b, true)` leaves two carriers with two `changed()`. | 11, 22.3 |

## The critic's three levers

1. **Closure, one rule:** the small-signal rule (11.1); 16.2 finished with a jump column and no `Ic` column (there is no such solve any more); 16.3 assigns every analysis.
2. **Operating-point edges, once:** held nodes (16); `settle` commit = ordinary commit at `(t_next, 0)` (15); seeds test `now() == 0`, which needed no event and no module.
3. **Syntax economy:** `final`, `Ic`, `Nodeset`, `at`'s period cut, plus `uic` and six unmapped variants; analysis-constant environment allowed in definitions.

Text removed because a rule now implies it: the "Arrive" step, the branch-kind row, the old step 5 of 16, the three-line lead of 11.1, `transition`'s `start`/`!analysis(Tran)` paths, the v1 accounting paragraphs of B.1, G.31.

## Counts

**Concepts** (table of section 0): v2 11 claimed / 12 effective (template rode on param). v3 **11**, with template inside the module rule, which decides it.

**Constructs** (B.1): v2 66 listed / 67 honest. v3 **65**, the `fn` `where` included in the forms row: − `final`. Not counted as tokens but also gone: `at`'s `period` parameter, eight `Analysis` variants (13 → 5), `uic`. Added: none (`starting`, `every` are stdlib, written out).

## Known weak points (for the critic)

1. `starting()` assumes an analysis starts at `t = 0`.
2. A bare `after` still holds in `Dc` (G.32); only `inertial` follows a sweep.
3. Distortion and PSS are host procedures over transients; periodic small-signal is not offered (16.3, G.30).
4. `Dc` of a circuit with an integrating node fails as singular rather than using the node's `= value`.
5. `transition` with an input that starts unknown steps, without a ramp, to the first known value.

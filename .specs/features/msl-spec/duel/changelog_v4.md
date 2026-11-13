# Changelog v3 → v4 (builder, round 4, final polish)

Answers `critique_v3.md` (CONVERGED, 0 BLOCKER, 1 MAJOR, 9 MINOR). M1 and all nine MINORs are fixed; nothing is rebutted. No concept and no construct was added; one stdlib function was deleted. The spec went from 148 129 to 147 369 bytes with two new pieces of semantics in it (16.4, periodic small-signal).

Not compiled or executed: no compiler exists. The changed stdlib modules (`transition`, `zi_nd`, `inertial`, `since`) were traced by hand against the rules.

## Items

| Item | Action | What changed | Where in `spec_v4.md` |
| --- | --- | --- | --- |
| **M1** continuation between analyses | fixed (critic's paragraph, as a section) | New 16.4, two cases and no third. *Continue a transient*: same analysis, later stop time; time, queue, discrete state, instance state and analog history kept; no operating point, nothing committed or held, `init()` does not fire, `analysis(Static)` never true. *Start from a transient's final state*: final discrete values are the `(0, 0)` commits, final potentials stand for the nodes' `= value` (so held in a `Tran`, by 16's existing rule), time restarts, queue dropped, core instance state fresh; the first commit is a change, so clocks re-arm. 16.1 rule 5 (the one clause) is deleted. The PSS row of 16.3 now rests on 16.4. | 16.4, 16.3, 15 (stop-time `settle` commit), D.2 |
| **m1** `starting()` is `analysis(Static)` | deleted | `starting()` is gone from 22.1, the "not core" list and B.1's stdlib list. `transition`, `zi_nd`, `inertial` test `analysis(Static)`. Every "`now() == 0`" explanation is gone (13 rule 6, 16, 16.1, 22.2, 22.5, G.32): zero left in the text. The time-origin assumption went with it. | 22.1, 22.2, 22.5, 13 rules 6–7, 16 |
| **m2** `analysis` has two regimes | fixed, said once | 11.1: `Static` "is the one variant that names a solve, not an analysis"; in a reaction it holds exactly at the instants of an operating point or a `Dc` point. The fn table of 3 now splits on the same line as 14 (constant during an analysis vs. environment of the solve). | 11.1, 3 |
| **m3** periodic small-signal refused | fixed (defined) | The small-signal rule now says what it is taken around: the operating point, or each point of a periodic transient. 16.3's row defines periodic small-signal and noise: discrete state is the trajectory's, `now()` is the point's time, event times move as in 23.3; the model sees `Tran` for the trajectory and `Ac`/`Noise` for the linearization. A general sampled system therefore has a response with nothing written. Removed from "out of scope"; G.30 rewritten; C.2 and G.6 say honestly that Piperine does not implement the linearization yet. | 11.1, 16.3, G.30, C.2, G.6 |
| **m4** `transition` unknown start ignores `td` | fixed | `x.known_or(0.0)` is read only under `analysis(Static)`; elsewhere `y0.known_or(0.0)`, `y1.known_or(0.0)`. The first known value arrives after `td`, with a ramp from 0.0. One `let` fewer. D.4's sentence corrected. | 22.2, D.4 |
| **m5** two corners of the `settle` commit | fixed | The one statement (15) now says the arriving solve reads "what `(t_next, 0)` will apply to that carrier, whichever commit wins there, and the definitions that follow from it". Step 3.4 is phrased by that ("a value the solve had not already read"), not by who made the commit. | 15 |
| **m6** node `= value`: hint vs. hold | fixed | 16 says plainly: a transient's operating point and a `Dc` of the same circuit differ on purpose for a held node with a DC path; a model wanting only a hint leaves it to the host's nodeset; a hold that contradicts a potential contribution is a singular solve naming both. | 16 |
| **m7** leftovers | fixed | `log` is instance-relative, not a third reserved name: the simulator's own records (unsettled loop, host `force`) go to the `log` stream of the instance concerned (19 rule 2; 8.2 rule 3; 23.1 rule 3). Constructs stay at 65. Section 3's table fixed (m2). 11 rule 3: "anything constant during a solve". | 19, 8.2, 23.1, 3, 11 |
| **m8** repetition | fixed | The `settle` commit is stated once, the bold paragraph of 15; the 13 table, 15 steps 1 and 4, 16.1 rule 2, 16.2, 17.2 and C.1 refer to it. 13 rule 6 is split: rule 6 is the first-commit rule only, rule 7 the three ways to say "the start". 5.2, 12.1, 16 step 2, 23.1 rule 4, E.1, E.2, D.6, 22.5 point at it instead of restating. Also merged or cut: the two bold paragraphs of section 0, 8.3 rule 1's restatement of the `let` rule, 15's restatement of 12.1 rules 1–3, 17.2 rule 5 (was 15.2 rule 6), a C.1 row, and the pre-duel rows of Appendix H (now one row pointing at `spec_v0.md`). | throughout |
| **m9** unsaid, small | fixed | A `settle` commit made at the stop time is dropped unless the transient is continued (15; noted on `Dut`). `since` returns `now() - t.known_or(-inf)`, readable in `analog`. A held node with no `ddt` moves freely in the first step, which the integrator takes as a restart (16). | 15, 19, 22.1, 16 |
| weak point 2 (bare `after` in `Dc`) | fixed as the critic suggested | G.32 gives the opt-out `q = e after if analysis(Tran) { tpd } else { 0.0 };` in place of "the same four lines". | G.32 |

## Remaining weak points (builder's own)

1. Periodic small-signal is defined but not implemented in the reference simulator (C.2, G.6); its definition leans on 23.3, which was written for gradients with respect to params, not perturbations of signals. The mathematics is the same; no model exercises it in the galleries.
2. 16.4 case 2 holds *every* node in a new `Tran`. That is 16's rule applied uniformly, but it means a circuit whose final state is inconsistent with its sources at `t = 0` fails as singular rather than relaxing.
3. `analysis(Tran) && analysis(Static)` still reads cryptically (one comment added in `CapIc`); no name was added for it, by the "no new names" instruction.

## Counts

| | v3 | v4 |
| --- | --- | --- |
| Concepts | 11 | 11 |
| Constructs (B.1) | 65 | 65 |
| Stdlib names | — | −1 (`starting`) |
| Size | 148 129 B | 147 369 B |
| Occurrences of `now() == 0` (or `<= 0`) / `starting()` | 7 / 12 | 0 / 1 (the Appendix H history row) |

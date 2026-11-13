# Changelog v4 → v5 (builder, round 5, last)

Answers `critique_v4.md` (CONVERGED, 0 BLOCKER, 2 MAJOR, 6 MINOR). Surgical text edits only: no concept, construct or stdlib name added or removed, no gallery model touched. Not compiled or executed (no compiler exists); nothing was re-traced beyond reading the edited sentences against their neighbours.

| Item | Action | What changed | Where in `spec_v5.md` |
| --- | --- | --- | --- |
| **M1** 16.4 case 2 | fixed (critic's rule) | "Start from a transient's final state" is deleted. One rule: an analysis run at a transient's last instant keeps everything (time, queue, discrete state, instance state, history); no operating point of its own, nothing committed again or held. A `Tran` there is the same transient continued; `Dc`/`Ac`/`Noise` there is one more instant where time does not advance. PSS is continued transients only; shooting is the simulator's procedure. | 16.4, 16.3 PSS row, Appendix H |
| **M2** sampled real value | fixed, stated once | 23.3 rule 4: a commit of a real value carries the sensitivity of the expression committed, at the commit's instant (plus slope × `dt*/dp` when the instant moves); any other type has none. The same rule is the periodic small-signal response. `SampleHold` (12.1) checked: it now has its zero-order-hold response, named in 11.1. Pointers only elsewhere. | 23.3 rule 4, rule 6 and closing example; 11.1 bullet; 5.1 rule 2; 16.3 row; G.30 |
| **m1** 16.2/11.1 periodic | fixed, one footnote | Under 16.2's first table: in a periodic analysis the last column is taken at each point (`now()` its time, real carriers per 23.3 rule 4); at a transient's last instant `now()` is that instant. 16.3 says `zi_nd`'s `delay` form in the linearization is on purpose. 11.1's table left as is. | 16.2, 16.3 |
| **m2** hold agreeing with a potential contribution | fixed | Always an error: a hold is a potential source; a loop of potential contributions is singular whether or not the values agree. | 16 |
| **m3** continuation details | partly fixed | Host `set`/drive between runs is a commit at the resume instant; the old stop time stays a time point. Continuing after a `finish` record: not addressed. | 16.4 |
| **m4** step 3.4 parenthesis | fixed | "(a `settle` commit is one it had, below)". | 15 |
| **m5** 13 rule 7 | fixed | Moved under the list as prose; 13 has seven rules; the one reference to "13 rule 7" now reads "(13)". | 13, 16 |
| **m6** 16's last paragraph | fixed | Restatement cut to a pointer at 11.1. | 16 |

## Left open

- A `Dc` run at a transient's last instant has equilibrium solves, so `analysis(Static)` is true there and `on settle && analysis(Static)` reactions (`zi_nd`'s seed) run; 16.1 rule 3's "dropped when the analysis ends" is not reconciled with "the queue stays" for commits made during that analysis.
- Continuing after `finish` (m3); the 11.1 table's "constant during an analysis" for a periodic analysis is covered only by 16.3's "the model sees" column.
- Periodic small-signal still has no traced gallery example and no implementation (C.2, G.6).

## Counts

| | v4 | v5 |
| --- | --- | --- |
| Concepts | 11 | 11 |
| Constructs (B.1) | 65 | 65 |
| Size | 147 369 B | 148 973 B |

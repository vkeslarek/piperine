# Adversarial spec duel — brief (read fully)

Working dir: /tmp/claude-1000/-home-keslarek-Git-piperine/570f7032-f229-4af2-937c-a6378b382406/scratchpad/spec-duel

`spec_v0.md` = the TO-BE spec of a Rust-like mixed-signal modeling language (Piperine's next frontend).
Two roles alternate:
- **CRITIC** reads the latest `spec_vN.md`, writes `critique_vN.md`.
- **BUILDER** reads `spec_vN.md` + `critique_vN.md`, writes `spec_v(N+1).md` (complete, self-contained spec — not a diff) + `changelog_v(N+1).md`.

## Goal (shared, non-negotiable)
Reach the SMALLEST set of abstractions/language concepts that is still expressive enough to
accept 100% of mixed-signal simulation: full analog (everything Verilog-A/AMS analog can
model: contributions, ddt/idt, noise, AC stimulus, limiting, events/crossings, operators like
transition/slew/absdelay/laplace/zi, hidden state, $bound_step, initial conditions, analysis-
dependent behavior), full digital (everything synthesizable + behavioral Verilog/SV can model
for simulation: 4-state/unknowns, multi-driver resolution & strengths, tristate, latches,
memories, FSMs, delays, races-free scheduling, X-propagation), and the mixed boundary (A2D/D2A,
connect modules/converters, event synchronization between solver and kernel), plus a minimal
elegant toolset (I/O, assertions, testbench/host hooks, parametrization, reuse of OSDI models).
Elegance = few orthogonal concepts, each with one rule; everything else is stdlib written IN
the language, not new syntax.

## Measuring
Every critique and every builder output must state the **concept count** (core concepts table)
and the **construct count** (new syntax beyond Rust: keywords, operators, attributes, macros).
Downward pressure on both. A concept is only justified by a concrete model that cannot be
written without it.

## CRITIC rules
- Find HOLES: concrete mixed-signal/analog/digital models that the spec cannot express, or
  expresses ambiguously, or whose semantics are underspecified (scheduling, convergence,
  event ordering, X semantics, multi-driver, small-signal/noise, hierarchy, params). For each:
  a 3–10 line model sketch that breaks it + why.
- Find BLOAT: concepts/constructs that are redundant, overlap, could be a stdlib module/fn, or
  whose rule has exceptions. Propose what to delete/merge.
- Find INCONSISTENCIES between sections.
- Rank: BLOCKER (100% goal fails) / MAJOR / MINOR. Max ~25 items, no padding. Be harsh,
  concrete, short. No praise.
- Verify previous round's fixes (from `changelog_vN.md` if present): say which items are
  CLOSED, which REOPENED.
- End with: concept count, construct count, and a line `VERDICT: CONVERGED` only if there are
  zero BLOCKERs and ≤2 MAJORs; else `VERDICT: CONTINUE`.

## BUILDER rules
- Address every BLOCKER and MAJOR (fix, or rebut in changelog with a concrete reason).
- Prefer DELETING/MERGING concepts and pushing features to stdlib over adding syntax.
  Adding a concept requires removing or merging another, or a written justification.
- Keep the spec complete and coherent: concepts table, semantics (execution model,
  scheduling, analysis modes), stdlib inventory, grammar of additions, galleries. Update
  galleries so they still compile under the new rules. Keep the original's style (terse,
  each concept one rule). Do not lose content the critic didn't attack unless simplifying it.
- `changelog_v(N+1).md`: table item → action (fixed/merged/deleted/rebutted) → where; then
  concept & construct counts before/after.

## Both
- Only write files inside the working dir. Don't edit anything else. Don't touch git.
- Optional reference for what real simulation needs: /home/keslarek/Git/piperine/VAMS-LRM-2-4.md
  (read-only, grep it, it's huge) and /home/keslarek/Git/piperine/docs/spec/.
- Final message: ≤10 lines — files written, counts, verdict/top items.

## Elegance score (added round 1)
CRITIC also scores the spec 0–10 on each axis, one-line justification each, then the mean:
1. Orthogonality — concepts don't overlap; no two ways to say one thing.
2. One rule per concept — no exceptions/special cases in the rules.
3. Syntax economy — constructs beyond Rust, each pulling its weight.
4. Stdlib-in-language — how much is written IN the language vs. magic builtins.
5. Readability — gallery models read like the circuit; a newcomer predicts behavior.
6. Semantic closure — every construct's behavior defined in every analysis (DC/AC/tran/noise/event).
The orchestrator scores independently with the same rubric.

## Convergence gate (updated round 2)
`VERDICT: CONVERGED` requires ALL of: zero BLOCKERs, ≤2 MAJORs, AND critic's elegance mean ≥ 8.0.
Otherwise `VERDICT: CONTINUE`, and the critic names the 2–3 changes that would raise the
lowest axes most.

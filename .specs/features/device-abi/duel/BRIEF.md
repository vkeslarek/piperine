# Adversarial ABI duel — brief (read fully)

Working dir: /tmp/claude-1000/-home-keslarek-Git-piperine/570f7032-f229-4af2-937c-a6378b382406/scratchpad/abi-duel
Repo (READ ONLY for you): /home/keslarek/Git/piperine-codegen-plan (main + codegen record)
and /home/keslarek/Git/piperine-solver-completeness (solver analysis + solver-restructure plan).

Two roles alternate:
- **BUILDER** writes `abi_vN.md` (complete, self-contained ABI spec — not a diff) + `changelog_vN.md` (N≥1).
- **CRITIC** reads the latest `abi_vN.md` (+ changelog), writes `critique_vN.md`.

## What the ABI is
The **device ABI of Piperine's solver**: the one interface through which every device couples to
the solver. Devices are (a) generated from MSL (elaborated POM → IR → Rust transpiler → compiled
`cdylib`; or the IR interpreter as the reference backend), and (b) EXTERNAL devices written by
others: e.g. an AVR microcontroller emulator, a co-simulated digital block, a compiled OSDI-like
analog model, a host-side stimulus. User: "Eu gostaria que o ABI fosse tão elegante quanto a
linguagem" — it must be as elegant as MSL (the language spec reached elegance 8.25 with 11
concepts: value/fn, nature, carrier, drive, param, module, let, analog, digital, event, instant).

## Binding context (do not relitigate)
- Read `.specs/features/codegen/context.md` (decisions C1–C10, open O1–O8) in the codegen-plan repo.
- Read `.specs/features/solver-completeness/{context,analysis,spec,design}.md` and
  `.specs/features/solver-restructure/{spec,design}.md` in the solver-completeness repo.
- Solver = one **event owner** (time/ticks, event queue, dispatch, commit order, rollback, event/record
  sink — a first-class debug surface) dispatching to an **analog sub-solver** (MNA, Newton, TrBdf2,
  convergence, AC, noise, the present discontinuity algorithm) and a **digital sub-solver**
  (Verilator-style levelized DAG over opaque-byte carriers, loops relax to convergence or fail).
  The two sub-solvers never see each other; **the boundary is events, always**. A digital event cuts
  an analog step only when it crosses the boundary.
- Carriers are **opaque bytes**; the solver holds physical complexity, never language complexity.
- The MSL spec in docs/spec (codegen-plan repo) defines the semantics the ABI must be able to carry:
  Part III (carriers, drives, resolve, extern modules §8.5 "osdi"/"cosim"), Part IV + §14 core
  primitives, Part V execution (instants (t,n), microsteps, settle, bound_step, analyses), Part VI io
  (emit/records), Part VIII host (introspection, live params, probes), Appendix C (reference
  algorithms), §15 invariants I1–I8 (determinism, snapshot, order, no trace of rejection).
- Generated code is compiled once per design (MD-18); rustc in the environment for now.
- Open O1 (Jacobian by forward AD with dual numbers vs symbolic) — the ABI may decide it.
- The ABI may break everything that exists today (user: "fica a vontade para mudar o ABI").

## Goal
The SMALLEST set of ABI concepts and entry points that still couples 100% of: MSL analog
(contributions, charges/ddt, idt, delays, noise incl. flicker/correlation, AC stimulus, limiting,
internal unknowns, bound_step, crossings, operating point/continuation), MSL digital (definitions,
reactions, drives, resolve, commits with transport, microsteps), the mixed boundary, every analysis
(DC/OP, AC, noise, transient, sweeps, the host procedures tf/pz/sp/sens/adjoint/pss), introspection
(names, params, live param changes), checkpoint/rollback, and EXTERNAL devices with their own time
base (an AVR emulator that advances to a time, raises events/interrupts, reads/writes pins) — with
versioning, safety across a `cdylib` boundary, and debuggability (the user left Cranelift because
low-level code was impossible to debug and verify).

## Measuring (every builder output and every critique)
- **Concept count** (table: concept → one rule) and **entry-point count** (functions/callbacks).
- **Elegance score 0–10** (critic gives it, with reasons): few orthogonal concepts, each with one
  rule, no special cases, symmetric analog/digital treatment, external and generated devices are the
  same thing, nothing the language already decides leaks into the ABI.
- Downward pressure on both counts; a concept is justified only by a concrete device that cannot be
  coupled without it.

## CRITIC rules
- HOLES: concrete devices that cannot be coupled or are underspecified (3–10 line sketch each).
- BLOAT: redundant concepts/entry points, special cases; propose merges/deletions.
- UNSAFETY/DEBUGGABILITY: UB across the dylib boundary, ownership, threading, versioning, how a
  user debugs a misbehaving device.
- Rank BLOCKER / MAJOR / MINOR, max ~25 items, no praise. Verify the previous round's fixes.
- End with the elegance score and the two counts.

## BUILDER rules
- Address every BLOCKER and MAJOR (fix or argue with a concrete reason); MINOR at discretion.
- Each version complete: concepts table, the exact Rust-side shape (traits/structs) AND the C ABI
  shape it lowers to across the dylib (or argue why one suffices), lifecycle, per-analysis call
  sequences, event semantics, worked examples: a resistor, a diode with limiting and noise, a
  capacitor (charge), a D flip-flop, a comparator crossing (analog→event), a DAC (event→analog),
  an AVR emulator as an external device.
- Changelog: what changed and why, item by item against the critique.

## Stop rule
Stop when the critic gives elegance ≥ 8 AND no BLOCKER remains, or after round 5 (then the
orchestrator decides). Final output: `abi_final.md` = the last builder version.

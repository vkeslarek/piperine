# MSL Fixes Context

**Gathered:** 2026-11-17
**Spec:** `.specs/features/msl-fixes/spec.md`
**Status:** Scope chosen by the user; X4 decided; Q1 to Q4 answered (2026-11-17). Spec and tasks approved.

Everything the user said reached the planner through the orchestrator. What arrived verbatim is quoted; what arrived as the orchestrator's statement of a decision is marked "relayed".

---

## Feature Boundary

Lane A's first feature after the 2026-11-17 audit (`GAPS.md`): the "Correctness fixes" cluster of `GAPS.md` §10 and the "Math and method catalogue" cluster, together. The user's words: "Correções + Catálogo juntas". It runs beside two other lanes that this feature does not touch except where `design.md` names a cross-lane touch: lane B `host-ii` (`piperine-api` and `piperine-cli` host actions, session options, analyses exposure, root wiring and integrated tests) and lane C `solver-hygiene` (solver numerics V1 to V11, error naming).

---

## Implementation Decisions

### Scope (user, 2026-11-17, relayed)

- Correctness fixes: S1, S5, B1, B2, L11, L17, L41, L46, L47, L48, L54, L55, each verified against the code; an item bigger than S to M, or one in another lane's area, moves out with the reason.
- The catalogue: L14 to L16 (the Appendix H rows of `f64`, of the integer types, of `Option` and of `bool`), then `db10`/`db20` and the stdlib and Book examples they unblock.
- S3 is a candidate when it is S to M and does not depend on X6.
- Semantics come from Appendix H (inherited Rust semantics); tests derive from the spec's stated results, not from the implementation.

### Binding rules (user, relayed)

- ngspice parity is the top priority: the 7 twins' goldens and the parity targets stay byte-identical; no tolerance is retuned.
- Fail loud rather than silent.
- The Book stays held to the binary; every Book example a fix unblocks runs.
- "Implementação sempre em Sonnet e verificação em Opus": every task's executor is Sonnet.
- The interpreter is frozen at the M1 analog subset (R1, `.specs/features/codegen/context.md`): fixes land on the Rust path; the interpreter gets a fix only when the gap exists there and the fix is trivial, otherwise it refuses by name.

### X4 — what `fatal` and `finish` do to a run (user, 2026-11-17)

The user chose "Encerra e falha (Recommended)". As relayed: `finish(msg)` ends the run successfully (the `sim` record has `failed = false`); `fatal(msg)` ends the run with `failed = true`; `piperine run` then exits 1 and `piperine test` reports `FAIL` showing the message.

Note: the stdlib `finish()` takes no message (§22.6.2: `pub fn finish() { emit!("sim", finish = true); }`); the decision reads as "a `finish` record ends the run without failing it". This feature does not add a parameter to `finish`.

### Decisions of other lanes, for awareness (user, 2026-11-17, relayed)

- X3: a `target` only reports; it never fails a test. Lane B.
- X9: a derived param is recomputed in generated code on a host `set`. Lane B (`host-ii`) plans it; if the recompute lands in `elab/` or `piperine-transpile` it is named there as a cross-lane touch. This feature does not plan it and avoids any design that would block it: nothing here folds a param into generated code beyond what elaboration folds today.

---

### Q1 to Q4 (user, 2026-11-17)

Verbatim: "Aceito todas (Recommended)". As relayed:

- Q1 (a): S3 is deferred to a feature of its own, with X6.
- Q2 (a): a `CROSS` watch whose value is NaN is unknown; abi.md A4 is amended.
- Q3 (a): the minimal solver touch, so an operating point carries its analysis's flag.
- Q4 (a): `bound_step` under an `if` or a `match` is refused by name now; it comes with L40 later.

---

## Approvals

- Spec: 2026-11-17 - user: "Topo" (lanes) and "Aceito todas (Recommended)" (Q1-Q4).
- Tasks: 2026-11-17 - user: "Pode fazer com Opus por agora, com Sonnet implementando e Opus verificando".

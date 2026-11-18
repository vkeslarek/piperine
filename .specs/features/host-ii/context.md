# Host II Context

**Gathered:** 2026-11-17
**Spec:** `.specs/features/host-ii/spec.md`
**Status:** The five decisions and the answers of 2026-11-17 bind (below). Spec and tasks approved.

Everything the user said reached the planner through the orchestrator. What arrived verbatim is quoted; what arrived as the orchestrator's statement of a decision is marked "relayed".

---

## Feature Boundary

The host library and the command line reach what the solver already does and the spec gives the host: session options, the small-signal analyses the device ABI runs today (transfer function, pole-zero, DC sensitivity) and the periodic steady state, metadata reads, host drives and timed `set`s from the Rust library, the `sim` and `target` records judged as the user decided, a digital-only transient, and a `set` that recomputes every value derived from the param it changes. The Book gets the new keys, analyses and command. The root package's integrated Book tests and the root `tests/e2e/` are the follow-up feature `root-e2e`.

---

## Implementation Decisions

### The scope (user, 2026-11-17, lane B)

- "Host II mais o wiring do root." The Host II cluster of `GAPS.md` (H1, H2, H3, H4, H6, H9, H11, L39, L56), then the root wiring of `ROADMAP.md` items 4 and 4b.
- 2026-11-16, for the root wiring: "Finalizando o cli + api na feature, pode fazer o wiring do root e criar os testes integrados. Eu recomendo que tudo que está no livro tenha ao menos 1 teste integrado."
- Relayed: PSS in a bench and on the command line, `analysis = "pss"` with `period` and `tstab`.
- Relayed: Python (B4) and the language server (B3) are out of scope; lane A (`msl-fixes`) and lane C (`solver-hygiene`) run in parallel and are not touched except where `design.md` names the touch.

### The five decisions (user, 2026-11-17, relayed by the orchestrator)

| # | Topic | The user's words | Decided |
| - | ----- | ---------------- | ------- |
| X9 | A derived param under a host `set` | "Recalcular no código gerado (Recommended)" | Honor §8.1.23: when a host `set` changes a param, every value derived from it (child params, constant lets, definitions) is recomputed by a function compiled into the device (generated Rust), with no re-elaboration and no recompile, so MD-18 stays intact. This closes `GAPS.md` S2. It touches lane A's area (elaboration output, plan, transpile); host-ii plans it, names the touch in `design.md` and keeps those tasks in a phase of their own, ordered so that they rebase cleanly on `msl-fixes` |
| X8 | The top's ports | "Só pela API host (Recommended)" | A top with ports may be driven by the host only through `piperine-api` (Rust host tests; Python later). Benches in MSL stay the command-line path; no manifest `drive` key for now |
| X3 | A `target` in `piperine test` | "Só informa (Recommended)" | A `target` only reports: `piperine test` prints the margins at the end and never fails on them; `require` is the constraint |
| X4 | `finish`, `fatal` and the `failed` field of `sim` | "Encerra e falha (Recommended)" | `finish` ends the run, ok (`failed = false`); `fatal` ends it with `failed = true`: exit 1, and `piperine test` prints `FAIL` with the message. Lane A implements the ending of the analysis under S1; host-ii only consumes the record |
| X7 | Names of the session options | relayed: the orchestrator's proposal, to which the user did not object | ngspice `.options` names, lowercase, as run keys with their flags: `reltol`, `abstol`, `vntol`, `trtol`, `temp`, `tnom`, plus `nodeset`. Time resolution and the bounds take ngspice-aligned names the spec lists |

### The answers to the spec's questions (user, 2026-11-17, relayed by the orchestrator)

Verbatim: "Aceito todas (Recommended)".

| # | Question | Decided |
| - | -------- | ------- |
| host-ii Q1 | metadata from the command line | a new command `piperine info [<run>]` |
| host-ii Q2 | the timed `set` surface | library only, `Session::set_at` |
| host-ii Q3 | small-signal keys | `input`/`output` for `tf` and `pz`; `wrt` for `sens` |
| host-ii | `tnom` | stays out until `simparam` (L47) |
| root-e2e Q1 | a Book claim | blocks, listings and every stated value or outcome |
| root-e2e Q2 | where the tests live | root `tests/book/` |
| root-e2e Q3 | the move | one move to `tests/e2e/<scenario>/`, no symlink |

### Approvals (user, relayed by the orchestrator)

- The host-ii spec, 2026-11-17: "Topo" (lanes) and "Aceito todas (Recommended)" (Q1-Q3, tnom).
- The host-ii tasks, 2026-11-17: "Pode fazer com Opus por agora, com Sonnet implementando e Opus verificando".
- The root-e2e spec, 2026-11-17: "Aceito todas (Recommended)"; its design and tasks come later.

### What earlier decisions already fix

- A `[run.<name>]` key has a flag of `piperine run`, and a flag wins (MD-65 item 2, `msl-cli` CLI-03). A new observation or analysis option is a run key with its flag (MD-66 item 2).
- Every rule of what a run prints or judges is a `piperine-api` method with a Rust test; the command line is a shell (MD-65 item 1).
- Elaboration fixes devices; a sweep or a `set` restamps the compiled circuit and never re-elaborates or recompiles (MD-18, MD-63 item 2).
- ngspice parity is measured: a tolerance lives only in `ToleranceContract`, with a rationale (MD-38).
- Measurements over a waveform are host code (MD-66 item 3).
- Tests live in the crate they exercise; root `tests/` is the host surface (MD-28).

### Agent's Discretion

Every technical choice: the shape of the new tables, the fixtures of the parity twins, the names of new types, how the derived-param function reaches the solver. Each is a row of the spec's Assumptions table or of the design's Tech Decisions, open to correction.

### Declined / Undiscussed Gray Areas

None declined. Every question was answered on 2026-11-17.

---

## Deferred Ideas

- The follow-up feature `root-e2e` (spec only, this branch): one integrated test per Book example and claim at the root, and the single root `tests/e2e/` with the ngspice twins moved under it.
- A follow-up host feature for what needs new solver mechanisms: `force` and `release`, a host wait on an event, continued transients and an analysis at a transient's last instant (H6), operating-point variables, noise (blocked by L45), S-parameters, adjoint gradients.


### F8, PSS with a digital drive (user, 2026-11-18)

Verbatim: "Mergear e passar ao periodic (Recommended)". HII-07 AC 4 is amended to an analog `now()` drive; PSS of designs whose digital state grows with time (stdlib `every`'s counter, `transition`'s `ramp.t0`) moves to `periodic-small-signal` (GAPS H15).

# ROADMAP.md — Piperine

**Rewritten 2026-11-21.** Forward-looking only: what is in development, what waits and in what
order. Delivery history lives in git, `.specs/STATE.md` and each feature's `verification.md`;
open defects live in `GAPS.md`.

Rules that hold for every item: **fail loud** (what the toolchain cannot do is a named error,
never a silent no-op); **code follows the spec** (a divergence is a defect, not a documented
state); **ngspice parity first** (twins under the single `ToleranceContract`; tolerances are
never retuned).

---

## North star

Piperine is one language and one toolchain for mixed-signal design: MSL describes the circuit
— analog, digital and the boundary between them — and the same project checks, builds, runs,
tests and verifies through the `piperine` command line and the `piperine-api` Rust library, on a
native solver cross-validated against ngspice.

---

## Development pool

Each feature lives in `.specs/features/<name>/`. Up to three lanes run at a time, each with its
own worktree and target directory. Implementation on Sonnet, verification on Opus, planning on
Opus or Fable; a feature merges to `main` and upstream when its verifier passes.

### In flight

| Feature | What it delivers | Progress | Lane |
| --- | --- | --- | --- |
| `composite-values` (+ `value-semantics`, MD-76) | Structs, tuples, payload enums, arrays and ranges, `Option` of aggregates, iterators (Appendix H.5–H.7), `normal`; value semantics: no borrows, moves, `Clone` or `Copy`, every `let` reassignable (`E0903`, `E0161`) | B1–B7 done; B8 implementing; then B9–B10, verifier, merge | C |
| `fix/long-transient-and-param-where` | Transients past ~4.5 s at the default 1 fs tick (a stopgap step floor, recorded in Appendix C.2); a host `set` or `--param` re-checks every dependent `where` against the params as they hold now | fixed and verified; fix round for the verifier's findings | A |

### Waiting, in order (all unblock when `composite-values` merges)

| # | Feature | What is left | Waits on |
| --- | --- | --- | --- |
| 1 | `generics` | Generic structs, enums, functions and traits, const generics, `Fn` values (L4, L7, L8, L29–L32, L58, S3, X6); tasks amended for value semantics | composite-values |
| 2 | `analog-operators` | B4–B7: `simparam`, `tnom`, noise sources and the noise analysis, the `laplace`/`zi` family (L43–L45, L47, L49, U7) | composite-values |
| 3 | `unknown-semantics-events` | B5: L51, L53 | composite-values, generics A–B |
| 4 | `params-and-io` | B5–B6: integer, enum and `Option` params, `E0122`, `parse_table`, `parse_hex`, `table_model` | composite-values |
| 5 | `resolved-nets` | B3 and later: resolved nets over composite natures (L21, L58) | composite-values, generics |
| 6 | `resolved-nets-bundles` | B2 and later: connecting and resolving bundles (L25) | composite-values, generics, resolved-nets |
| 7 | `unknown-semantics-per-level` | B2–B6: unknowns per level (S6, G6), the `KNOWN_LEVELS` ABI flag | B1 delivered |
| 8 | `code-clarity` | Readability and structure refactor under the seven cleanliness rules, CC-11 included (no language special cases: builtins declared in the MSL prelude), and the split of the three oversized `pom/{resolve,reflect,lower}/mod.rs`; a Fable inventory first | items 1–7 |
| 9 | `spec-closure` (**item 13**) | Fable closure of spec ↔ implementation; G1, G2, G4, G5, X1, X2, H13 and the discrepancies logged during the run | code-clarity |

**The automatic run stops after item 13** for the user's review of the closure, the spec and the
code (user, 2026-11-18 and 2026-11-21).

### After the review

| Feature | What it delivers | State |
| --- | --- | --- |
| `transient-performance` | Faster transients (Fable diagnosis and plan): step control around events, stamps and node lists reused, no instant at an idle step; T25: the analog time as an integer tick epoch plus a small offset, replacing the long-transient stopgap | spec, design and tasks on `feature/transient-performance`; first after the review (user, 2026-11-21) |
| `host-iii` | `force`, `release`, waits for an event, continued transients (H1, H6) | spec |
| `host-analyses` | S-parameters, Fourier, distortion, operating-point variables (H2) | spec |
| `sweeps-montecarlo-optimization` | Multi-axis sweeps, Monte Carlo, optimization (H2, H7) | spec |
| `gradients` | Adjoint gradients (H8, `E0807`–`E0809`) | spec |
| `cli-packages` | Packages from the command line (H10) | spec |
| `conformance` | The galleries and conformance on the compiled backend (U1, U2, U4, U6, U8, U9) | spec |
| `foreign-models` | Foreign and OSDI models, linking (L12, L33) | spec |
| `multi-rate` | Digital edges that do not cut analog steps (V12) | spec; later (user) |
| `surrogate-models` | Averaged or surrogate models for long runs (V13) | spec; later (user) |
| `periodic-small-signal` | Small-signal analyses over the periodic steady state (V16) | spec |
| `ngspice-models` | The ngspice device models in the MSL standard library — same names, params and equations, each with a parity twin — so a SPICE user moves over easily | **idea only**; well after the rest (user, 2026-11-21) |
| `python-host` | A Python host | deferred until the host features are done (user, 2026-11-18) |
| `lang-server` | The language server on MSL (broken today) | last (user) |

---

## Decisions waiting for the user

Logged during the unattended run; presented at the item 13 review.

| Topic | Question |
| --- | --- |
| Resolved nets | The `driving` field in the device ABI |
| Unknowns per level | The per-level known-byte asserts in the buck; the `f64` mask |
| Spec §13 | D2 |
| Book | The clipper twin |
| Buck example | Brown-out OVP, start-up peak iL 1.78 A, the 12 → 9 V ramp of 236 µs |
| `transient-performance` | Decision points 1–5 of its plan |
| Elaboration | A child's node connected to another placement (`P0001`) is a lowering gap, not a spec rule: close it, or spec it |

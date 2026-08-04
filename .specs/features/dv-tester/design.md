# dv-tester Design

**Spec:** `spec.md` (DVT-01..45). **Surface design:** `ideal.md` in this
directory — read it first; it carries the level taxonomy, the four level-1
families, the cost model and the decision log (§12). This document covers **where
things live and how they are built**; that one covers what they look like.
**Vision:** `../design-verification/ideal.md` §4.
**Depends on:** `dv-core` (`validation_reports()` is how a tester speaks).
**Follow-on:** `../codegen-simplification/` — this feature extracts, that one
delegates.

> Rewritten 2026-07-31 against `ideal.md`'s third revision. The previous version
> placed every component in `piperine-api` and treated the tester as the subject;
> the subject is now a host-language **device library**, and its mechanism half
> lives in a new crate.

## A. Shape

Three levels, three homes, one `Element` at the bottom (`ideal.md` §1).

```
PHDL declaration                          host implementation
────────────────────                      ───────────────────────────────
@device pub mod LoadTransient(            class LoadTransient(pip.Sequencer):
    inout out : Electrical,                   def build(self, b):
);                                                b.isource("out", "gnd",
                                                            wave=Wave.const(10e-3))
   │  (one concept, two binding modes)          def run(self, t):
   │   plugin → cdylib, TOFU-pinned                 yield t.advance(100e-6)
   └── host  → in-process registration             d = t.arm(Measure.min("out"))
                                                    ...
                       │
        ┌──────────────┴───────────────────────────────────┐
        ▼                                                   ▼
  piperine-api                                       piperine-signal  (new, pure)
  ─────────────────────────────                      ─────────────────────────────
  HostElement  ──blanket adapter──▶ Element          Wave      value from time
  StampSink / Ctx / StateView                        Behavior  value from state + ∂
  Sequencer   (coroutine driver)                     events    crossing detection
  level-1 → stamp adapter                            Measure   streaming reductions
        │                                            ops/limit (extracted §3.9)
        ▼
  piperine-solver — sees only `Element`, plus one new seam:
                    a dynamic digital subscriber (§C5)
```

The solver gains **one** mechanism (§C5) and otherwise does not change. Codegen
is not touched by this feature at all — its copies of the extracted families stay
until `codegen-simplification` removes them.

## B. Component breakdown

| Component | Home | Notes |
|---|---|---|
| `Wave` descriptors, evaluation, breakpoints, DC values | `piperine-signal` | new; per-kind DC value follows SPICE convention (`ideal.md` §12.10) |
| `Behavior` kinds + closed-form Jacobians | `piperine-signal` | new; ~8 kinds + composition |
| `verify_derivatives` FD oracle | `piperine-signal` | new; the guard on every hand-written Jacobian |
| Runtime ops (`delay`/`slew`/`transition`/`idt`) | `piperine-signal` | **extracted** from `codegen/device/analog/operators.rs` |
| Event detectors (`cross`/`above`/`timer`) | `piperine-signal` | **extracted** from `codegen/device/analog/events.rs` |
| `table` | `piperine-signal` | **extracted** from `codegen/resolve/pom/analog_ops.rs` |
| Limiter (pnjlim/fetlim) | `piperine-signal` | **extracted** from `codegen/device/analog/limits.rs` |
| Crossing tier 2 (predict + bound + report) | `piperine-signal` | new; §C6 |
| `Measure` accumulators, gating, error bounds | `piperine-signal` | new |
| `HostElement` + blanket `Element` adapter | `piperine-api` | §C2 |
| `StampSink` / `Ctx` / `StateView` | `piperine-api` | the FFI-safety work; §C2 |
| Level-1 → stamp adapter | `piperine-api` | turns `Wave`/`Behavior` into MNA rows; surfaces the limiter's veto |
| `@device` host binding mode | `piperine-api` + `piperine-lang` | §C3 |
| `Sequencer` (coroutine driver, wakes, findings) | `piperine-api` | §C4 |
| Dynamic digital subscriber | `piperine-solver` | §C5 — the only solver change |
| PyO3 bridge | `piperine-python` | duck-typed method presence, one catch point |
| Python surface | `piperine-python` | thin; `pip.Device`/`Wave`/`Behavior`/`Measure`/`Sequencer` |
| Monitors | no new code | ordinary `digital` modules + `dv-core`'s channel |
| `cover` parse + POM | `piperine-lang` | statement kind inside a `constraint` block |
| `cover` bin mapper | `piperine-codegen` | reuses `dv-core`'s pointwise path |
| `cover` posture + bin cap | `piperine-solver` | `cover=on\|off` (D9); raisable cap (D10) |
| Coverage merge | `piperine-api` | host state across runs and seeds |

## C. The decisions that shape the build

### C1. Extraction first, and the cross-check harness is not optional

The four extracted families (§B) are the crate's first commits, and their
correctness bar is **"the existing tests still pass, relocated"** — a far stronger
starting position than greenfield. But between this feature and
`codegen-simplification`, each family exists **twice**.

The holding measure is a **per-family cross-check fixture**: one fixture, both
paths (codegen's JIT device and the crate's direct call), asserted equal. Four
fixtures, authored here, deleted by the follow-on when codegen's copy goes away.

This is a hard requirement (DVT-06), not hygiene. Without it the copies drift and
`codegen-simplification` stops being "delete a copy" and becomes "reconcile two
behaviors" — a much worse job, discovered late.

**Ordering consequence:** extraction (Phase 1) precedes everything, because every
later phase consumes it and because a relocation is easiest to verify when
nothing else has moved.

### C2. The blanket adapter is the whole ABI story

`HostElement` mirrors `Element`'s defaulted shape (two required methods, the rest
defaulted). A blanket `impl<T: HostElement> Element for T` presents it to the
solver, so **no solver code learns that host devices exist** (MD-01: one ABI, no
downcast).

The real work is not the trait — it is `StampSink`/`Ctx`/`StateView`, the safe
wrappers that expose matrix rows and solution values across an FFI boundary
without handing Python a raw pointer. Design rules:

- The sink is **index-validated**: an out-of-range row/column is a loud typed
  error, never undefined behavior.
- Views are **borrow-scoped** to one call; nothing a host device holds can
  outlive the solve step that produced it.
- Nothing exposes a `&mut` to the matrix itself — only `g(row, col, v)` /
  `i(row, v)` / `branch(...)` operations.

`limiting_report` rides here too: a level-1 limiter can veto convergence
(`ideal.md` §3.9 D4), so the adapter surfaces that signal alongside stamps rather
than swallowing it.

### C3. `@device` gains a binding mode, not a new attribute

`@device` already exists (`headers/device_port.phdl`, MD-24): a PHDL module that
declares ports and params and defers its implementation. A host device is that,
with the implementation registered in-process instead of loaded from a cdylib.

Build consequence: **the PHDL side barely changes.** The elaborator already
accepts `@device`; what is new is a second resolution path at circuit-build time
— when `CircuitCompiler` meets an `@device` module, it looks for a plugin binding
first and a host registration second, and fails loud naming both if neither is
found. The binding mode is *inferred from where the implementation is*, never
spelled differently in PHDL (`ideal.md` §8.7).

**No new macro** (MD-13 rule 5). A Rust host device implements `HostElement` and
registers explicitly; Python uses a `@pip.device` decorator because that is how
registration is spelled in that language. `#[pip::device]`'s `inventory`
collection stays a plugin-binary mechanism and is not reused in-process.

### C4. The coroutine resume seam

`advance(dt)` inserts a breakpoint at `t + dt`; the sequencer is resumed **after
step acceptance**, never inside the accept/reject decision (D11). That ordering
is an acceptance criterion (DVT-25), not an implementation detail: a sequencer
resumed during a step that is later rejected breaks the model with no clean
repair.

Language asymmetry, stated rather than hidden: Python uses a generator; Rust has
no stable generators, so a Rust sequencer implements `resume` as an explicit
state machine. The *contract* is identical — same wake sources, same findings,
same ordering — and only the authoring ergonomics differ.

The cost D11 buys is accepted and named: forcing the solver to land on every
sequencer breakpoint constrains its timestep, so a fine-grained tester runs
slower. A known inefficiency beats an unknown failure mode.

### C5. The dynamic digital subscriber — the one solver change

`t.wait(edge("clk", RISING))` needs the digital scheduler to resume a waiter when
a net changes. Today sensitivity is static and compiled; nothing can subscribe at
runtime.

The seam is deliberately small: **a subscriber list consulted after net
settling.** Not a new event kind — `EventKind::Digital` already carries the edge
with `priority: Exact`. Not a new scheduling concept — the scheduler already
settles and already emits. What is added is a registration point and a
post-settle dispatch.

Two rules ride along:

- **After settling only** (`ideal.md` §3.5/§12.3). A mid-delta-cycle value can be
  an artifact of DAG evaluation order rather than a signal the circuit had;
  waking on one produces results that move when an unrelated edit reorders the
  graph.
- **The seam is not tester-only.** An in-language monitor and a live-plotting
  host would both want it. A one-consumer mechanism in the solver is a smell;
  this one has three plausible consumers, which is the argument for putting it in
  the solver rather than faking it in the api crate.

Sequence this **after** time wakes: examples A–E and G in `ideal.md` are provable
without it, so the feature has a working spine before the scheduler is touched.

### C6. Crossings: tier 2, with the error published

The audit (`ideal.md` §3.4) found that PHDL's own `@cross` fires at the first
accepted point *after* the crossing — late by up to a timestep, unreported — and
that nothing in the ABI can force a retry at a computed instant.

Level 1 ships **tier 2**: extrapolate `t_cross` from the trajectory, cap the next
step through `bound_step_hint`/`$bound_step` (a seam that already exists end to
end), re-detect, iterate; publish the residual error. No solver change, strictly
better than today, and never a bare plausible instant.

Tier 3 (exact) needs `EventKind::Crossing` promoted from `Advisory` to `Exact`
plus reject-and-retry. That is a scoped solver feature with its own spec, and the
existing enums are already the right shape to receive it.

## D. Data flow: `t.arm(Measure.min("out"))` then a load step

1. **`arm`** constructs a `piperine-signal` accumulator and registers it with the
   host device; a handle returns to the coroutine. No solver involvement.
2. **`install`** replaces the driving `Wave` at the current instant
   (install-relative time, §`ideal.md` 4.2) and registers the discontinuity as an
   `Exact` breakpoint.
3. **`advance(50e-6)`** yields; the sequencer declares a breakpoint at
   `t + 50 µs` through `next_breakpoints`.
4. The solver runs. Every **accepted** point feeds `(t, v)` into the accumulator
   through the adapter — natively, zero host calls. Rejected steps feed nothing.
5. At the breakpoint, after acceptance, the coroutine resumes. `droop.value`
   reads the accumulator; `droop.error_bound` reads the LTE-derived bound.
6. `t.expect(...)` compares **against the bound** and emits a finding through
   `dv-core`'s channel with time and instance provenance.

Host calls in that whole sequence: one, at step 5. That is the design.

## E. Risks

| Risk | Mitigation |
|---|---|
| The two copies of A–D drift before delegation lands | §C1 — four cross-check fixtures, a hard requirement (DVT-06) |
| Extraction changes a number | The numeric oracles already exist: `tests/ngspice_validation.rs` (30 tests) plus each family's relocated behavior tests. Nothing here may change a single value |
| Exposing the stamp matrix to Python unsoundly | §C2 — index-validated, borrow-scoped, no raw pointers, loud on invalid indices |
| A hand-written `Behavior` Jacobian is wrong | `verify_derivatives` (DVT-12); a new kind is not done until it passes |
| A sequencer is resumed on a step later rejected | §C4 — resume strictly after acceptance, as an AC (DVT-25), with a recording sequencer proving it |
| Breakpoint density destroys transient performance | Accepted and documented (D11); measured on a realistic test so the cost is known rather than discovered |
| Host crossings leak into the Newton loop | Counted-call test asserting zero crossings between breakpoints (DVT-31) |
| `Measure` returns a plausible wrong overshoot | Error bound published by default; `expect` compares against the bound (DVT-15) |
| The digital subscriber becomes a one-consumer seam | §C5 — three plausible consumers named; if it turns out to be tester-only, that is a signal the design was wrong, not a detail |
| **The GIL**: a Python device holds it during the solve | Documented as a property of the Python tier; level 1 sidesteps it entirely, which is the main reason the catalog exists |
| Python exceptions mid-solve | One catch point in the bridge; typed error carrying simulation time; analysis stops cleanly |
| Authors integrating state in `load_*` instead of `accept_timestep` | `ideal.md` §7 is documentation *and* an example; the trap already exists for PHDL authors |
| Ideal drive causes convergence failures | Declared impedance is the default shape; ideal force explicit; discontinuities register breakpoints |
| Two devices driving one net | Loud conflict, not last-writer-wins, regardless of instantiation order |
| A coroutine that never yields | Bounded zero-advance resumes per instant, then loud |
| Scope: `cover` is a different subject riding along | It is P3 and separable — the natural cut line if this feature needs splitting |

## F. Test strategy

| Layer | Targets |
|---|---|
| `piperine-signal` purity | a guard asserting the crate's in-tree dependency list is **empty** and that no `Element`/matrix/Cranelift symbol appears — the crate boundary tested mechanically (MD-31: a policy invariant lives in the gate) |
| Extracted families | the relocated unit tests, unchanged; plus four cross-check fixtures against codegen's copy |
| `Behavior` | `verify_derivatives` over a sample grid per kind; a deliberately wrong Jacobian must be caught |
| `Measure` | online-vs-post-run agreement on one `Waveform`; an overshoot fixture whose analytic peak must lie inside the reported bound; `refine=True` narrows it; loud on unfed/never-gated reads |
| Crossings | tier-2 convergence against an analytically known crossing instant, with the published error shrinking as the step floor drops |
| ABI | a resistor written three ways (PHDL / Rust host / Python host), instantiated twice in a hierarchy, identical operating points |
| Sequencer | resumed exactly at declared instants, never on a rejected step; two sequencers interleaving; zero host calls between breakpoints (counted) |
| Edge waits | a clock-synchronized burst; a waiter proven to resume only after settling |
| Analog drive | a ramp into an RC against the analytic segment response; install-relative timing; a replacement's edge registered |
| Loud paths | AC with a sequencer; unknown net; driver conflict; non-yielding coroutine; rebuild-class `set_param`; unfed measure |
| Parity | `host_parity.rs`-style enumeration; the four twinned devices (DVT-37) |
| Coverage | bins filled by a sweep with empty bins named; two runs merge; oversized cross refused at elaboration |

## G. Spec-document updates (`docs/spec/`)

| Document | What changes |
|---|---|
| `part_viii_host_api.md` + `appendix_c_host_surface.md` | **the largest documentation surface in the three dv- features** — the whole library: the three levels and their costs, `Wave`/`Behavior`/`Measure` catalogs, the `Sequencer` and its one `wait`, `install` semantics, measure lifecycle and error bounds, posture interaction, reach rules (D14), and the closed-Python/open-Rust asymmetry |
| `part_vii_solver.md` | the `HostElement`/`Element` blanket adapter; the dynamic digital subscriber and its settled-only rule; resume-strictly-after-acceptance (D11); drive segments and the impedance requirement; the crossing fidelity tiers with today's tier-1 behavior stated as a **correction to the existing `@cross` documentation**; the AC refusal (D13) |
| `part_i_language.md` | `@device`'s host binding mode; `cover` as a `constraint`-block statement kind; `cover` joining the reserved words |
| `appendix_b_grammar.md` | `CoverStmt` production, bins and set forms |
| `part_ii_elaboration.md` | `cover` bin resolution and the elaboration-time cross-product cap (D10) |
| `appendix_a_worked_examples.md` | a worked host-device example **and** a worked tester example — this is the feature most likely to be learned from an example rather than a grammar table |

A new crate also needs its own entry wherever the crate table lives
(`CLAUDE.md`'s "Crate responsibilities", `ARCHITECTURE.md` if present) — a crate
absent from the map is a crate nobody finds.

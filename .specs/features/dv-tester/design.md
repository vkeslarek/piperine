# dv-tester Design

**Spec:** `spec.md` (DVT-01..23). **Vision:** `../design-verification/ideal.md` §4.
**Surface design:** `ideal.md` in this directory — the library API, layer by
layer, with an implementability review. Read it before writing any of layer 1;
this document covers where things live, that one covers what they look like.
**Depends on:** `dv-core` (`validation_reports()` is how a tester speaks).

## A. Shape

No grammar. A tester is an element on one side and a host program on the other.

```
host (Python generator, or Rust)                 solver
─────────────────────────────────                ──────────────────────────────
def uart_echo(t):
    t.drive("rst", 1)
    yield t.advance(100e-9)  ──── installs breakpoint at t+100ns ───▶
                                                 next_breakpoints() → [t+100ns]
                                                 (core/element.rs:187, exists)
                                                 … solve to the breakpoint …
                                                 step ACCEPTED
    ◀──── resume exactly once ──────────────────  callback
    t.drive("rst", 0)
    t.ramp("vdd", 0, 1.8, 1e-6) ── installs a stamp SEGMENT ────────▶
                                                 element stamps from the segment
                                                 through every Newton iteration
                                                 with NO host crossing (§C2)
    got = yield from t.uart_recv(...)
    t.expect(got == byte, …) ──── Error finding ─▶ validation_reports() (dv-core)
```

The scheduling half already exists. What is new is the resume callback, the drive
segments, and the host API.

## B. Component breakdown

| Component | Home | Notes |
|---|---|---|
| Sequencer element | `piperine-solver`-facing element in `piperine-api` | implements `Element`; `next_breakpoints` + `EMITS_VALIDATION` |
| Resume callback contract | `piperine-api/src/` | language-agnostic (D15); Python and Rust both implement |
| Drive segments | the sequencer element | piecewise description stamped between breakpoints (§C2) |
| Host API | `piperine-api/src/` + `piperine-python/src/` | `advance`, `drive_voltage`/`drive_current`/`ramp`, digital `drive`, `read_voltage`/`read_port`, `expect`/`warn` |
| Generator adapter | `piperine-python/src/` | ergonomics only — one contract underneath |
| Monitors | no new code | ordinary `digital` modules + `dv-core`'s channel |
| `cover` parse + POM | `piperine-lang/src/parse/`, `pom/` | statement kind inside a `constraint` block |
| Bin mapper | `piperine-codegen/src/kernel/analog/` | reuses `dv-core`'s pointwise path; a hit is a counter increment |
| `cover` posture + bin cap | `piperine-solver/src/analyses/context.rs` | `cover=on\|off` (D9); raisable cap (D10) |
| Coverage merge | `piperine-api/src/` | host state across runs and seeds |

## C. The four decisions that shape the implementation

### C1. Who chooses the instants decides the language

This is the rule that separates a tester from a `constraint`, and it corrects the
coarser "inside the time loop → in the language":

| Instants chosen by | Must live | Rate |
|---|---|---|
| the **solver** (every accepted point) | compiled, in-kernel | 10⁶–10⁹ evaluations |
| the **test** (its own breakpoints) | the host, imperative | 10⁴–10⁵ wake-ups |

At ~1 µs a host crossing, 10⁵ wake-ups is noise and 10⁹ is fatal. A `constraint`
is in the kernel because the solver dictates its rhythm; a tester can be Python
because it dictates its own. Same time loop, different rate.

`SimHooks` is explicitly *not* the hook — it is coarse
(`transform_design`/`before_lower`/`after_solve`) with no per-step site. A tester
is an `Element`.

### C2. Analog drive: the host installs segments, the element stamps them

A stamp value must exist during every Newton iteration, and calling the host from
inside that loop is exactly what §C1 forbids.

So `t.ramp(net, from, to, dur)` installs a **segment** at the current breakpoint,
and the element stamps from that description until the next one — no host crossing
in between. This is how a PWL source already behaves; the tester generalizes it
under host control. Python decides *what* to drive; the element decides *how* to
stamp.

Two requirements ride along:

- **Declared impedance.** `drive_voltage(net, v, rout=50.0)` stamps a Norton
  source. An ideal force stays available — it is a legitimate bring-up tool — but
  it is spelled explicitly and never defaulted, or the tester reintroduces the
  silent ideality this project criticizes `wreal` for.
- **Registered discontinuities.** An ideal step must register its breakpoint so
  the integrator sees the edge instead of interpolating across it. The digital
  path already lives on discontinuities, so this is existing solver business.

Test that the invariant holds: count host calls between breakpoints and assert
zero (DVT-11).

### C3. Rollback: explicit advance *is* the guarantee

The hard problem is that the solver rejects timesteps and a Python generator
cannot un-advance.

The answer (D11): `advance(dt)` inserts a breakpoint at `t + dt` and asserts that
nothing the tester observes changes inside that window. The tester is resumed
only at that breakpoint, which is an accepted point by construction. No rollback
protocol, no speculative state, no un-advancing — the failure mode is designed
out rather than handled.

The cost is accepted and named: forcing the solver to land on every tester
breakpoint constrains its timestep, so a fine-grained tester makes the transient
slower than it would otherwise be. A known inefficiency beats an unknown failure
mode.

Implementation consequence: the resume site must be **after** step acceptance,
never inside the accept/reject decision. A tester resumed during a step that is
later rejected breaks the model with no clean repair, so this ordering is an
acceptance criterion (DVT-03), not an implementation detail.

### C4. Coverage is the one construct whose value is accumulation

Every other construct here belongs to one analysis. Coverage belongs to a whole
regression, which produces three design differences:

1. **Its own posture** (D9). Riding `checks=` would let `dv-gradients`' optimizer
   pollute the coverage database with 10³ inner-loop iterates nobody meant as
   verification runs. `cover=on|off` on `Context`.
2. **Elaboration-time bin cap** (D10). Bin edges are literal ranges, so a cross's
   joint bin count is known statically — a 10⁶-bin cross is refused before
   anything allocates. Raisable on `Context` so a legitimately large cross is one
   explicit line rather than a compiler fork. A runtime warning was the
   alternative and loses to the project's fail-loud rule.
3. **Host-side merged state.** Counters merge across runs and seeds and persist
   between sessions. The kernel side is cheaper than a margin — a bin hit is a
   counter increment with no signed distance and no argmin tracking.

## D. What the tester makes unnecessary

Recorded here because it is a design decision, not a scope accident (D16):

- **RNM as a testbench component** — abstract neighbor, stimulus generator,
  boundary checker — is fully replaced by this feature. Any stimulus at all,
  imperative, in the host, with declared drive impedance.
- **RNM as internal block abstraction** is mostly covered by writing the abstract
  model as an ordinary simple `analog`/`digital` module; the large speedup comes
  from not simulating transistors, which a simple module already gets.
- **Most in-language monitors.** Protocol checking is overwhelmingly at clock
  edges — the tester's rhythm — so the monitor residue is narrow: continuous
  obligations at the solver's rhythm, or checks that should ship with a block.

Net effect on the language: the `behavioral` body kind left with RNM, dropping the
V1 grammar delta from five additions to four. A capability decision made the
language smaller.

## E. Risks

| Risk | Mitigation |
|---|---|
| A tester is resumed on a step that is later rejected | §C3 — resume strictly after acceptance, as an AC (DVT-03), with a recording tester proving it |
| Breakpoint density destroys transient performance | Accepted and documented (D11); measured on a realistic test so the cost is known rather than discovered |
| Host crossings leak into the Newton loop | §C2 — counted-call test asserting zero crossings between breakpoints (DVT-11) |
| Ideal drive causes convergence failures | Declared impedance is the default shape; ideal force explicit; discontinuities register breakpoints |
| A tester in AC silently does nothing | Loud refusal (D13, DVT-15); DC either defined or loud, never guessed |
| Non-reproducible runs from tester side effects | Allowed by D12, but documented as the author's responsibility and called out as the one link `(seed, index)` does not cover |
| Two testers or a tester and a design source drive one net | Loud conflict, not last-writer-wins (Edge Cases) |
| A generator that never yields hangs the simulation | Bounded zero-advance resumes per instant, then loud |
| The `Element` ABI has no imperative surface for "wait" | The scheduling half exists (`next_breakpoints`); the resume callback is the new part, and this feature is where ABI gaps will surface — sequence it after `dv-core`'s channel lands |

## F. Test strategy

| Layer | Targets |
|---|---|
| Solver/element | `piperine-solver/tests/` or `piperine-api/tests/` — resumed exactly at declared instants, never on a rejected step, multiple testers interleaving, zero host calls between breakpoints |
| Analog drive | ramp into an RC compared against the analytic segment response; ideal step registers its breakpoint; internal-net drive appears in findings |
| Host API | `piperine-python/tests/` + root `tests/` — UART-echo generator over thousands of cycles, posture behavior (`strict` aborts on first mismatch, `collect` records all), Rust twin produces identical findings |
| Monitors | a hierarchical fixture where a monitor sees what the top-level tester cannot |
| Coverage | bins filled by a sweep with empty bins named; two runs merge rather than replace; oversized cross refused at elaboration |
| Loud paths | tester + AC; unknown net; driver conflict; non-yielding generator |

## G. Spec-document updates (`docs/spec/`)

| Document | What changes |
|---|---|
| `part_viii_host_api.md` + `appendix_c_host_surface.md` | **the largest documentation surface in the three features** — the whole tester API: the request protocol, every primitive, the resume contract, posture interaction, and the reach rules (D14). This is a host-language API, so the host-API Part is its normative home |
| `part_vii_solver.md` | the sequencer element's contract: wake sources (time breakpoints, digital-event wakes, analog crossings), resume-strictly-after-acceptance (D11), drive segments and their impedance requirement, and the AC refusal (D13) |
| `part_i_language.md` | `cover` as a `constraint`-block statement kind, and `cover` joining the reserved-word list |
| `appendix_b_grammar.md` | `CoverStmt` production, bins and set forms |
| `part_ii_elaboration.md` | `cover` bin resolution and the elaboration-time cross-product cap (D10) |
| `appendix_a_worked_examples.md` | a worked tester example — this is the feature most likely to be *learned* from an example rather than a grammar table |

Note the asymmetry: `dv-core` and `dv-gradients` mostly extend language and
solver Parts, while this feature's normative content is almost entirely
host-API. That makes `part_viii_host_api.md`'s exclusion from both mkdocs navs
(surfaced by `p6-cleanup-architecture` T13, still open) a real obstacle for this
feature specifically — the documentation it produces would not be published.

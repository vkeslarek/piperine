# Digital network JIT + the stable event interface

Status (2026-07-05): the stable event interface (§3) and the **fused
combinational network compiler + runtime (§4)** are BUILT and tested — a cone of
pure-combinational digital instances compiles into one Cranelift function
(`NetworkComb`) driven by a `DigitalNetwork` that is one `DigitalEventModel`.
Test: `spec_simulation::digital_network_fuses_combinational_chain` (two inverters
fused, settled in one rank-ordered pass). Still open: (a) wiring the network into
the circuit's `run_digital_at` (cone detection + fallback), and (b) fusing
clocked/register members (comb-only for now — clocked instances stay per-device).

## Why

Today each digital module compiles to its own JIT kernel (`jit/digital/`:
`comb`/`seq`/`watch`), and the circuit drives them one device at a time through
the `Device::eval_discrete` trait method, inside an event/delta-cycle loop
(`solver/topology.rs`). That is already native per module, but every delta cycle
pays: dynamic dispatch per device, an ABI pointer-table marshal per call, and a
`BinaryHeap` event round-trip between devices that are actually just wires in a
combinational cone.

Verilator's insight: a synchronous digital network is a **DAG of combinational
logic between registers**. Rank the nodes into levels, evaluate the whole cone
in topological order in one straight-line pass, and only re-converge (settle)
where real feedback exists. We already compute the ranking
(`DigitalTopology`); the win is to **fuse the ranked network into a single JIT
function** so a delta cycle is one native call over shared buffers, not N
dispatched calls with N heap round-trips.

Two things must stay decoupled while we do this:

1. **The event boundary** — how the digital world exchanges value-changes with
   everything that is *not* part of the fused cone: the analog engine (A2D/D2A),
   and **external co-simulators** (an Arduino core, an ESP32 model, a
   hand-written C peripheral, things we haven't imagined). Events are that
   boundary. It must be a small, stable, documented contract — an "OSDI for
   digital" — so an external model is a first-class citizen, not a special case.
2. **The internal net evaluation** — the fused JIT cone. Free to change; nobody
   outside depends on its shape.

## Architecture (layers)

```
            ┌─────────────────────────────────────────────────────────┐
            │  Event scheduler  (solver/topology.rs: DigitalState)     │
            │  • time-ordered event queue, delta cycles, checkpoints   │
            └──────────────▲───────────────────────────▲──────────────┘
                           │ DigitalEventModel          │ DigitalEventModel
                           │ (stable contract, §3)      │
        ┌──────────────────┴───────────┐   ┌────────────┴───────────────┐
        │  Fused JIT network (§4)       │   │  External co-sim (§5)       │
        │  • one Cranelift fn over the  │   │  • Arduino / ESP32 / custom │
        │    ranked combinational DAG   │   │  • FFI or in-proc model     │
        │  • registers + settle loop    │   │  • sees ONLY events         │
        └───────────────────────────────┘   └─────────────────────────────┘
                           ▲
                           │ A2D voltages / D2A vars
                    ┌──────┴───────┐
                    │ Analog engine │
                    └───────────────┘
```

The scheduler owns net state and time. It never knows whether a model behind the
boundary is JIT-compiled Piperine logic or a socket to a running ESP32 image — it
only speaks `DigitalEventModel`.

## §3 The stable event interface (`solver/digital_interface.rs`)

The contract, minimal and versionable:

```rust
pub trait DigitalEventModel: Send {
    fn boundary(&self) -> &DigitalPorts;              // input/output nets
    fn init(&mut self, ctx: &InitCtx, sink: &mut dyn EventSink);
    fn evaluate(&mut self, ctx: &EvalCtx, sink: &mut dyn EventSink);
    fn samples_analog(&self) -> bool { false }        // A2D dependence
}
```

- **`DigitalPorts`** — the model's sensitivity list (`inputs`) and driven nets
  (`outputs`), as `DigitalNet` ids allocated by the circuit builder. This is the
  wiring; the scheduler uses it to decide who to wake on a change.
- **`EventSink`** — a write-only façade over the scheduler's queue
  (`fn emit(&mut self, net, value, delay)`), so a model never names the concrete
  `BinaryHeap<Reverse<DigitalEvent>>`. Swapping the queue, batching, or routing a
  model's events over FFI is invisible to the model.
- **`EvalCtx`** — read-only snapshot: `time`, current `nets: &[LogicValue]`,
  `analog: &[f64]` (A2D). No `&mut` to circuit internals.
- **`DigitalEvent`** stays the wire format: `(time, net, value, source, seq)`.
  `seq` is the intra-timestep tiebreaker; `source` is provenance. **This struct
  is the ABI** — additive changes only.

The current `Device::{eval_discrete, digital_input_nets, digital_output_nets,
digital_init, samples_analog}` methods are exactly this contract inlined into the
unified `Device` trait. Terrain step: the trait above is defined and the existing
`DigitalInstance` is shown to satisfy it via a thin adapter, so the scheduler can
be migrated to iterate `&mut dyn DigitalEventModel` without touching analog.

### Why this is the OSDI analogue

OSDI froze the *analog* device contract (eval residual/Jacobian/charge over a
flat ABI) so any compiled model plugs into any simulator. `DigitalEventModel`
freezes the *digital* contract (react to net changes → emit net events over a
flat event ABI) with the same intent: a compiled Piperine cone, a vendor's
gate-level netlist, or a firmware emulator are interchangeable behind it.

## §4 The fused network JIT (the follow-up feature)

Scaffold: `jit/digital/network.rs` (`DigitalNetwork`).

Build steps (Verilator-shaped):
1. **Collect** the connected digital cone: all instances whose nets are only
   driven/read by other digital instances (the boundary nets — those touching
   analog or an external model — become the fused kernel's *ports*).
2. **Rank** using `DigitalTopology::build` (already: topo order + back edges).
   Levelize; back edges mark genuine combinational feedback → settle regions.
3. **Fuse & compile**: emit one Cranelift function that, in rank order, inlines
   each instance's `comb` body (register `seq` bodies run in a separate clocked
   pass keyed by the edge/`fired` mask, as today). Per-instance variable banks
   become slices of one network-wide bank addressed by compile-time base +
   layout offset. Net values live in one `i64` array indexed by `DigitalNet`.
4. **Settle**: straight-line for acyclic ranks; a bounded fixpoint loop only
   around back-edge SCCs, with a delta-cycle cap (reuse the current 1000 guard
   and the "possible combinational loop" diagnostic).
5. **Boundary events**: after a settle pass, diff the *port* output nets against
   their pre-pass values and `emit` one event each. Internal net churn never
   leaves the kernel — that is the whole speedup.

Contract the fused kernel exposes upward: it *is* one `DigitalEventModel` whose
`boundary()` is the cone's ports. The scheduler treats the entire fused cone as a
single model. Registers, clock edges, and `@initial` keep today's semantics
(SPEC §9/§10.4).

Open design choices for the follow-up (not decided here):
- Whether `seq` fuses too or stays per-instance keyed by `fired`.
- SCC settle: Gauss-Seidel over ranks vs. re-emit into a mini-queue.
- One monolithic Cranelift fn vs. one fn per rank-level (helps the function-size
  limit — see the analog CSE note in `jit/emit.rs`).
- 4-state (`Quad`) vs. 2-state fast path when a cone is provably X/Z-free.

## §5 External co-simulators

An external model (Arduino/ESP32/unknown) implements `DigitalEventModel`
directly — in-process (a Rust shim over an emulator) or across FFI (a C ABI
mirroring `DigitalEvent` + two callbacks: `evaluate(time, nets*) -> events*` and
`init`). Because the scheduler only sees the trait, such a model coexists with
fused Piperine cones and the analog engine with no special path. Time
synchronization is the scheduler's existing event-time ordering; a co-sim that
runs ahead schedules future-dated events, one that lags is driven by input
events at its boundary nets.

This is the reason the boundary is being pinned down *before* the fusion work:
the fused kernel and an ESP32 image must be the same kind of thing to the
scheduler, or we will have baked the JIT into the core and made external models
second-class.

## Files

- `solver/src/digital.rs` — `LogicValue`, `DigitalNet`, `DigitalEvent` (wire ABI).
- `solver/src/digital_interface.rs` — **`DigitalEventModel`, `EventSink`,
  `DigitalPorts`, `EvalCtx`** (this contract). *(terrain)*
- `solver/src/topology.rs` — `DigitalState` (scheduler), `DigitalTopology`
  (ranking, back edges), `evaluate_dag_ordered`.
- `codegen/src/jit/digital/` — per-module kernels (`comb`/`seq`/`watch`) today.
- `codegen/src/jit/digital/network.rs` — **`DigitalNetwork`** fused-kernel
  scaffold + compile seam. *(terrain)*
- `codegen/src/device/digital.rs` — `DigitalInstance`, the per-instance driver
  (one `DigitalEventModel` today).

# solver-simplification Design

Macro → micro. This document fixes the target architecture, the per-layer
responsibility contracts, the `Element` decomposition, and the config
consolidation, then lists the phased tasks. **It stops at design** — no code
moves until the user approves (spec decision, 2026-07-18).

All design decisions are **locked** (user, 2026-07-18): module scheme **B**,
`SignalBridge` **folded**, `Element` **supertrait** decomposition (MD-01
amended), dead flags **removed**, `math/unit.rs` **removed**, `CircuitInstance`
**split by responsibility**, and Part VII **rewritten** as the canonical source
(finalization).

---

## 1. Macro — the layer map

The solver has five concentric layers. Today they are mostly right; this
feature makes each one's contract explicit and stops names/types from leaking
across boundaries.

| Layer | Module(s) | **Contains** | **Must NOT contain** |
|---|---|---|---|
| **Contract (ABI)** | `core/` | The `Element` object model, `ElementCapabilities`, `Net` naming, `Stamp`, introspection descriptors, `CircuitInstance`. The vocabulary every participant and every driver speaks. | Any analysis algorithm; any numeric schedule; any host-only result formatting. |
| **Numerics** | `math/` | Pure math with no circuit knowledge: linear algebra (`faer`), integration formulae (`TrBdf2`), Newton kernel, circular buffers. **`unit.rs` removed** (§11) — it was `f64` aliases, no type safety. | Anything that knows what an analysis *is*, or names a specific analysis; the dead unit aliases. |
| **Analog engine** | `analog/` | `Netlist`, `AnalogReference`, MNA assembly primitives — the analog view of the circuit. | Digital scheduling; per-analysis drivers. |
| **Digital engine** | `digital/` | Scheduler, topology, state, events, the two-phase delta cycle, the write-only `EventSink` façade. | Analog stamping; MNA. |
| **Analyses** | `analyses/` (renamed — §2) | One module per analysis: its request/setup types, its driver state machine, its result. Plus the shared convergence machinery (Newton/homotopy/stepper strategies) in a `convergence` sub-module. | Cross-analysis special-casing; raw numeric literals (they live in config). |
| **Config** | `config.rs` (new — §5) | Every solver tunable as typed, defaulted, documented fields: tolerances, plan limits, homotopy schedules, stepper gains, trace flags. | Logic. It is data. |
| **Public surface** | `abi.rs` + `prelude.rs` | Re-exports only (MD-17). | Definitions. |

**The crossing points** (analog↔digital) get one named owner each, so the
mixed-signal seam is not smeared across drivers:

- **D2A / A2D at accept time** → `SignalBridge` (or its replacement, ▶ §6): the
  *only* place analog accept-hooks seed the digital queue and run the settle.
- **Capability-gated ordering** → `CircuitInstance::capabilities()` drives
  whether a driver runs the mixed-signal loop at all. No driver probes devices
  by hand.

Rule: a driver in `analyses/` may call *down* into `analog/`, `digital/`,
`math/`, and read `config`. It may **not** reach sideways into another
analysis, nor up into the host.

---

## 2. The per-analysis module layout — Scheme B (locked)

Today: `analysis/<x>.rs` (state+options+result) **and** `solver/<x>.rs`
(driver) — same filename, two trees, boundary unnamed. **Locked: Scheme B**
(user: "sem mazes, colocalização fica mais organizado").

**B — Co-locate under `analyses/`.** One file per analysis (`analyses/dc.rs`,
`tran.rs`, `ac.rs`, `noise.rs`, `tf.rs`, `sens.rs`, `pss.rs`) holding **both**
its data types and its driver, with the data-vs-driver boundary preserved as
two documented `//` sections inside the file (`// ── request/state (what
element+host exchange) ──` / `// ── driver (how it runs) ──`). Shared machinery
→ `analyses/convergence.rs` (Newton/homotopy/stepper strategies + config),
`analyses/mod.rs` (`Context`, `Tolerances`, `Policy`). A maintainer opens
`analyses/tran.rs` and sees everything transient — one place, no parallel-name
maze, no `contracts/` directory.

The old `solver/` and `analysis/` trees dissolve into `analyses/`;
`solver/uic.rs` and `solver/solve.rs` fold into the analysis or shared modules
they serve.

Regardless of scheme, the **element-facing state** (`DcAnalysisState`,
`TransientAnalysisState/Context` — "what the element sees while stamping") is
re-exported through `abi`, and the **host-facing** options/results through
`prelude` (MD-17 unchanged).

---

## 3. Micro — `Element` decomposed into concern supertraits (SS-B, MD-01 amend)

One object, four legible concerns, no downcast. `Element` becomes the
conjunction of concern-scoped supertraits; capability flags still gate which
concern actually runs.

```rust
/// Analog participation: MNA loading + the analog lifecycle/convergence hooks.
pub trait AnalogDevice: Send + Sync {
    fn load_dc(...) -> Vec<Stamp<..,f64>> { Vec::new() }
    fn load_ac(...) -> Vec<Stamp<..,Complex64>> { Vec::new() }
    fn load_transient(...) -> Vec<Stamp<..,f64>> { Vec::new() }
    fn noise_current_psd(...) -> Vec<Noise> { Vec::new() }
    fn limiting_active(&self) -> bool { false }
    fn convergence_hint(&self) -> Option<ConvergenceHint> { None }
    fn bound_step_hint(&self) -> f64 { f64::INFINITY }
    fn next_breakpoints(..) -> Vec<Second> { Vec::new() }
    fn initial_conditions(&self) -> Vec<..> { Vec::new() }
    fn allocate_unknowns(..) {}
    fn set_temperature(&mut self, _t: f64) {}
    fn update(..) {}
    fn suggest_transient_step(..) -> Option<f64> { None }
}

/// Digital participation: two-phase delta cycle + hidden-state round-trip.
pub trait DigitalDevice: Send + Sync {
    fn boundary(&self) -> DigitalPorts<'_> { .. }
    fn init(&mut self, _sink: &mut dyn EventSink) {}
    fn seq_phase(..) -> bool { false }
    fn comb_phase(..) {}
    fn evaluate(..) { self.seq_phase(..); self.comb_phase(..); }
    fn has_input_on(..) -> bool { .. }
    fn digital_hidden_snapshot(&self) -> Option<(Vec<i64>,Vec<f64>)> { None }
    fn digital_hidden_restore(&mut self, _s: &(Vec<i64>,Vec<f64>)) {}
}

/// OSDI-style introspection: parameters, queries, terminals, opvars.
pub trait Introspect: Send + Sync {
    fn list_params(&self) -> Vec<ParamDescriptor> { Vec::new() }
    fn get_param(..) -> Option<Value> { None }
    fn set_param(..) -> Result<Invalidation, ParamError> { Err(..) }
    fn list_queries(..) -> Vec<QueryDescriptor> { .. }   // default via read_opvars
    fn query(..) -> Option<Value> { .. }
    fn list_terminals(..) -> Vec<TerminalDescriptor> { Vec::new() }
    fn read_opvars(&self) -> Vec<(String,f64)> { Vec::new() }
}

/// The single object the solver simulates. Identity + capabilities + the
/// cross-cutting lifecycle that isn't purely one concern.
pub trait Element: AnalogDevice + DigitalDevice + Introspect {
    fn name(&self) -> &str;
    fn capabilities(&self) -> ElementCapabilities;
    fn setup(&mut self, _ctx: &Context) -> Result<()> { Ok(()) }
    fn destroy(&mut self) {}
    fn accept_timestep(..) {}          // analog→digital bridge hook
    fn runtime_banks(&self) -> (&[f64],&[f64]) { (&[],&[]) }
}
```

**Cost, stated honestly:** a device that today writes one `impl Element` now
writes up to four impl blocks. All methods keep defaults, so a pure resistor is
`impl AnalogDevice for R { load_dc … }` + `impl DigitalDevice for R {}` +
`impl Introspect for R {}` + `impl Element for R { name, capabilities }` — the
empty ones are one-liners, and their presence *documents* that the resistor is
deliberately digital-inert (rule 1: contracts first). Codegen emits one device
impl, so its cost is mechanical. **No derive macro** (MD-13 rule 5) — the empty
blocks stay explicit.

**MD-01 amendment (locked, user 2026-07-18):** "One `Element` ABI, no downcast"
is preserved in full — still one object, still no `Any`. Amended to: *`Element`
is the conjunction of concern-scoped supertraits (`AnalogDevice` +
`DigitalDevice` + `Introspect`); the object is not split, only its surface is
grouped so each concern is separately legible.* The solver never names a
supertrait to select behavior — capabilities gate, as before. Rationale beyond
legibility: a downcast-based facet split would block the future **C-style ABI**
(user) — supertraits keep the object flat across the FFI boundary.

Cross-crate blast radius: `codegen/device/mod.rs` (the `PiperineDevice` impl),
`piperine-osdi` (external), plugin test doubles. All re-group their one
`impl Element` into the four blocks; no logic changes.

---

## 4. Micro — decompose `transient::solve()` (SS-C)

The 310-line `solve()` becomes a thin loop over named phase methods on
`TransientSolver`, each independently readable and testable:

```
solve():
  compute_initial_conditions()          // existing
  loop until t_end:
    phase = predict_step(dt)            // predictor seed + source/set updates
    step  = attempt_step(phase, dt)     // Newton solve at candidate dt
    match assess_step(step):            // LTE + breakpoint landing check
      Accept => { accept_step(); settle_digital(); snapshot(); dt = propose_dt() }
      Reject => { dt = stepper.reject_dt(); rollback() }
```

Each of `predict_step`, `attempt_step`, `assess_step`, `accept_step`,
`settle_digital`, `snapshot`, `propose_dt` is an owned method ≤ ~60 lines. No
numeric behavior changes; the phase boundaries follow the existing control flow
exactly (proven by transient parity, SS-06).

---

## 5. Micro — one config home (SS-D, rule 6)

New `solver/config.rs` (or `analyses/config.rs` under scheme B) holds the typed
schedule/gain/trace config. Homotopy strategies and the stepper read fields
instead of literals. **Every default equals today's literal** (SS-09 parity).

```rust
pub struct GminSchedule {          // GminStepping
    pub start_g: f64,              // 0.1
    pub decade_factor: f64,        // 0.1  (initial multiplicative step)
    pub relax_growth: f64,         // 1.3
    pub relax_cap: f64,            // 0.5
    pub backoff_growth: f64,       // 3.0
    pub backoff_cap: f64,          // 0.7
    pub max_steps: usize,          // 200
    pub floor_margin: f64,         // 10.0  (× gmin_floor)
}
pub struct SourceSchedule {        // SourceStepping
    pub knee_gmin: f64,            // 1e-6
    pub start_step: f64,           // 0.1
    pub step_growth: f64,          // 1.5
    pub step_cap: f64,             // 0.25
    pub min_step: f64,             // 1e-6
    pub max_steps: usize,          // 300
}
pub struct StepperGains {          // PiController — kp/ki already fields; add:
    pub kp: f64, pub ki: f64,      // 0.7 / 0.4
    pub grow_factor: f64,          // 1.5 (no-error growth)
    pub reject_divisor: f64,       // 8.0
    pub factor_clamp: (f64, f64),  // (0.2, 1.5)
}
pub struct TraceFlags {            // replaces PIPERINE_TRACE_*
    pub gmin: bool, pub source: bool, pub transient: bool,
}
```

Ownership: schedules live on `ConvergencePlan` (it owns the strategies);
`StepperGains` on the `TransientSolver`'s stepper; `TraceFlags` on `Context` or
`Policy`. `PlanLimits` (already the "hidden-constants home") absorbs or sits
beside these — one config family, discoverable from one place. Env vars may
remain as an *override* that seeds `TraceFlags`, but the code path reads the
typed field (SS-08).

---

## 6. Micro — dead surface removal (SS-E) & ▶ SignalBridge

**Remove (roadmap-confirmed no consumer):**
- `ElementCapabilities::LINEAR` (never produced, never read) — delete.
- `ANALYTIC_JACOBIAN`, `STAMPS_CHARGE` — delete the flags, the codegen
  producers (`codegen/device/mod.rs:163,165`), and the test asserts
  (`codegen/tests/codegen_api.rs:93,94,112,113`).
- `SUPPORTS_ROLLBACK` doc block promising `checkpoint_state`/`rollback_state`/
  `commit_state` — those methods don't exist. Delete the flag + phantom doc
  (the real lifecycle is `solver-commit-rollback`, out of scope). If a future
  reader needs the reservation, one `// reserved: solver-commit-rollback` line,
  no method promise.
- `SUPPORTS_QUERIES` — audit: keep only if a consumer reads it; else same
  treatment.

**`SignalBridge` — fold (locked).** It is a zero-field struct whose two methods
(`build_accept_state`, `settle`) are pure accept-hook→queue plumbing — *not* a
mixed-signal device path (any `Element` is natively mixed-signal, MD-01). Fold
both into `CircuitInstance` as named mixed-signal-seam methods (§10) and delete
the struct: one fewer indirection, the seam still has one named owner.

---

## 6b. `CircuitInstance` grouped by responsibility (SS-G)

Today `CircuitInstance` exposes 24 public methods spanning five unrelated jobs.
No new types are forced; the surface is **grouped and contracted** so each
method belongs to a stated responsibility (rule 1: the type reads as a
specification of what it is). The responsibilities:

| Responsibility | Methods | Contract |
|---|---|---|
| **Circuit state** | `netlist`, `nets`, `digital_label`, `capabilities`, `all_devices[_mut]` | Read-only views of the built circuit's structure. |
| **Analysis entry** | `dc`, `ac`, `transient`, `noise`, `transfer_function`, `sens`, `pss` | Hand a driver a borrow of the circuit + a `Context`. Uniform shape, one line each. |
| **Mixed-signal seam** | `init_digital`, `run_digital_at[_with_analog]`, `accept_and_run_digital`, `rebuild_digital_topology`, + folded `build_accept_state`/`settle` (SS-12) | The *one* place analog acceptance seeds digital events and the scheduler runs. Named seam methods, not a hollow `SignalBridge`. |
| **Live mutation** | `set_element_param`, `apply_convergence_hints`, `update_all`, `setup_all` | The MD-18 restamp path + per-solve hooks. |
| **Construction** | (none — stays in `CircuitBuilder`) | `CircuitInstance` gains no ad-hoc constructor beyond `from_devices_and_netlist` (builder output) and documented re-entry. |

Mechanically: reorder the `impl` block into these five contracted sections with
`// ──` headers; move the mixed-signal-seam methods together and absorb the
`SignalBridge` bodies; add a struct-level `//!`/doc contract naming the five
jobs. The analysis-factory methods may optionally move behind a small
`Analyses<'_>` accessor if the review prefers `circuit.analyses().tran(...)`
over seven inherent methods — **default: keep them inherent** (fewer types,
same clarity). Behavior unchanged (SS-16); this is surface organization.

## 6c. Remove `math/unit.rs` (SS-H)

`unit.rs` is 22 `pub type X = f64` aliases (`Volt`, `Ohm`, `Second`, `Siemens`,
`Farad`, …) — a remnant of the abandoned typed-units ambition. They give **zero**
compile-time safety (all are `f64`) and cost a module + an import at ~38
sites, including the `Second` in the `Element::next_breakpoints` ABI and the
`abi.rs` re-export.

Removal: delete `math/unit.rs` and its `mod unit;`, replace each alias with
`f64` at every use-site (`Tolerances { gmin: f64, min_res: f64, … }`,
`next_breakpoints(from: f64, horizon: f64)`, `math/constant.rs`, `solver/mod.rs`,
etc.), drop the `abi`/`prelude` `Second` re-export. Pure name removal —
numerics bit-identical (SS-17). The `abi` surface loses the `Second` alias; a
one-line note in Part VII §3.3 records that ABI times are `f64` seconds.

## 6d. Part VII as the canonical source (SS-I — finalization)

`docs/spec/part_vii_solver.md` is the last task: after every structural change
lands, Part VII is rewritten so it is the single complete, consistent source of
what the solver does. It is a **finalization** task — it runs last so it
describes the solver as actually built, not as planned.

Method — a section-by-section stated-vs-code audit:

- **§2 Circuit** → the grouped `CircuitInstance` responsibilities (6b) and
  `CircuitBuilder` construction. Remove any drift.
- **§3–4 Element ABI** → the composed supertraits (`AnalogDevice` /
  `DigitalDevice` / `Introspect` / `Element`, §3); drop the removed flags
  (`LINEAR`/`ANALYTIC_JACOBIAN`/`STAMPS_CHARGE`) and the phantom rollback
  methods; record ABI times as `f64` seconds (6c).
- **§8–13 algorithms** (MNA, DC, transient, AC, noise, TF) → verify each stated
  algorithm against the driver; the transient §10 reflects the decomposed
  phase methods (§4) and the PI/TR-BDF2 constants now living in config (§5).
- **§14 mixed-signal** → the folded seam (6b), one settle owner.
- **§15 convergence aids** → the config-homed homotopy schedules (§5), the
  strategy composition as shipped.
- **§16 failure rules** → cross-check every rule still fires in code.

Consistency pass: one vocabulary and one naming scheme matching the code, no
reference to a removed/phantom construct, every public contract and analysis
algorithm present. This is the feature's finalization gate (SS-18).

## 7. Map matches territory (SS-F)

- `STATE.md` MD-05: mark `NewtonStrategy`/`StepperStrategy` **done** (shipped:
  `DampedNewton`, `PiController`; wired in `dc.rs`/`transient.rs`).
- Add MD-01 amendment line (§3). Add a new MD for the config home if the user
  wants it locked.
- Each solver module gets a one-line `//!` responsibility contract matching the
  §1 table.

---

## 8. Phased tasks (for tasks.md after approval)

Ordered low-risk → high-churn, each phase ends green; parity gate after every
structural move. Part VII is deliberately **last** (describes the built solver).

- **P0 — Safety net.** Pin parity baselines (divider op, clipper tran,
  coupled-LC, mixed-signal divider, diode DC) as exact-value regression tests
  *before* touching structure. The refactor's oracle. (~2 tasks)
- **P1 — Dead surface (SS-10,11,12).** Remove `LINEAR`/`ANALYTIC_JACOBIAN`/
  `STAMPS_CHARGE` + codegen producers + asserts; delete phantom rollback doc;
  fold `SignalBridge` into `CircuitInstance`. (~4 tasks)
- **P2 — Remove `math/unit.rs` (SS-17).** Inline `f64` at all sites; drop the
  `Second` re-export. Parity trivially exact. (~2 tasks)
- **P3 — Config home (SS-07,08,09).** Extract `config.rs`; move every homotopy/
  stepper literal; route trace flags off env vars. Parity: numerics unchanged.
  (~4 tasks)
- **P4 — Element decomposition (SS-03,04).** Split into supertraits; re-group
  every `impl` across codegen/plugins/doubles; land the MD-01 amendment. (~6
  tasks)
- **P5 — `CircuitInstance` grouped (SS-15,16).** Reorder into the five
  contracted responsibilities; absorb the seam methods. (~3 tasks)
- **P6 — Module layout, Scheme B (SS-01,02,14).** Collapse `solver/` +
  `analysis/` into `analyses/`; per-analysis in-file sections; layer `//!`
  contracts. Pure moves + re-exports. (~5 tasks)
- **P7 — Transient loop (SS-05,06).** Decompose `solve()` into named phase
  methods. Parity: transient bit-identical. (~4 tasks)
- **P8 — Map (SS-13).** STATE.md MD-05 + MD-01 amendment + module docs current.
  (~2 tasks)
- **P9 — Part VII canonical rewrite (SS-18) — finalization.** Section-by-section
  stated-vs-code audit (6d); consistency + completeness pass; final full-suite +
  ngspice gate. (~3 tasks)

~35 tasks → **> 8-task budget: sub-agent delegation offer applies at Execute**
(≈5 workers, one per whole-phase batch).

## 9. Verification strategy

A pure refactor resists mutation testing, so the oracle is **behavioral
parity** + **contract assertions**:

- **Parity (primary):** P0 baselines must stay bit-identical through every
  phase. The existing 509-test suite is the broad net; the pinned exact-value
  tests are the sharp net.
- **Contract tests (new):** (1) every `ElementCapabilities` flag has a producer
  and a consumer (a test enumerating flags against known setters/readers);
  (2) each config default equals its former literal (parity by construction);
  (3) `Element: AnalogDevice + DigitalDevice + Introspect` compiles for a
  minimal hand-written test double implementing only `AnalogDevice` non-trivially.
- **Gate:** `cargo test --workspace` + `cargo build --workspace` zero warnings +
  ngspice live, matching baseline counts.

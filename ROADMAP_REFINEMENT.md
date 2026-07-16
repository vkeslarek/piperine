# ROADMAP_REFINEMENT.md — implementation-ready refinement of every open item

Companion to `ROADMAP.md` (2026-07-11). Every open item except `$plot`
(deliberately excluded — it will be done together with a GUI), refined to the
point where an implementer needs zero additional design decisions: **why** the
item exists, **where** the change goes, **what** to change, the **rationale**
behind the shape of the change, and the **test** that proves it. Read the
referenced files before editing them; line numbers drift, names don't.

House rules that apply to every item here:

- **Fail loud.** Nothing silently becomes `0.0` or a no-op. If a step of an
  item can't be finished, its unfinished half must produce a typed error.
- **No macro magic.** Data tables + plain helpers; every helper has an owner.
- **One model.** Structure flows through the POM (`piperine-lang::pom`);
  codegen's resolved form (`piperine-codegen/src/lower/`) is private.
- Closing a bench item updates `crates/piperine-bench/docs/SPEC.md` §11 in the
  same change. Closing a language item updates the relevant
  `docs/spec/part_*.md` section.

---

## Part A — Bench

### A1. The uniform API (G12): public Rust `load()` / `op` / `tran` / `ac` / `noise`

**Why.** Today the only way to run an analysis is to write a PHDL `bench` and
go through `BenchRunner`. Programmatic users (test harnesses, optimizers,
future Python bindings) need a typed Rust surface with the *same shape* as the
bench tasks, per bench spec §8.

**Where.**
- `crates/piperine-bench/src/session.rs` — `SimSession` is already the engine;
  it stays private-ish but gains a public facade.
- New file `crates/piperine-bench/src/api.rs` — the public surface.
- `crates/piperine-bench/src/lib.rs` — re-export.

**What.**
1. Define typed config structs mirroring the prelude bundles exactly
   (`OpConfig`, `TranConfig`, `AcConfig`, `NoiseConfig`) with `Default`
   implementations equal to the prelude defaults. They already exist implicitly
   as `Value::Record` parsing in `tasks.rs` (`solver_config`, `required_real`);
   the structs make that parsing the *shared* path: `tasks.rs` converts
   `Value::Record → TranConfig` and then calls the same session method the API
   calls. One decode function per config, owned by the config struct
   (`TranConfig::from_value(&Value) -> Result<Self, EvalError>`).
2. `pub fn load(source: &str) -> Result<Sim, miette::Report>` — wraps
   `parse_and_elaborate` + `SimSession` construction for the design's
   `top_module` (error if none). `Sim` wraps `SimSession` and exposes
   `op(&OpConfig)`, `tran(&TranConfig)`, `ac(&AcConfig)`,
   `noise(&NoiseConfig)`, each returning the existing result objects
   (`OpResult`, `Trace`, …) — public, not `Value::Object`-wrapped.
3. The **identical-shape review gate**: for each analysis, the bench task body
   must become `config-decode → self.api_method(cfg)`. If a bench task does
   anything the API method doesn't, the item is not done.

**Rationale.** One engine, two frontends (PHDL bench, Rust). Decoding
`Value::Record` into the struct at the task boundary keeps the interpreter's
dynamic world at the edge. Python/pyo3 is explicitly *after* this settles —
do not add it in the same change.

**Test.** New `crates/piperine-bench/tests/api.rs`: load the `CIRCUIT` fixture
from `tests/bench.rs`, run `op` via the Rust API, assert node voltage equals
the bench-driven result bit-for-bit.

### A2. `extract` / `.attach` / `.meta` as spice-plugin bench tasks (G13)

**Why.** Waveform post-processing (rise time, gain extraction, attaching
metadata to results) was spec'd as plugin territory; the plugin system now
exists, so these land as `PluginBenchTask`s in the official spice plugin.

**Where.**
- the external spice plugin repo's `plugin/src/lib.rs` — register the tasks.
- `crates/piperine-lang/src/eval/tasks.rs` — nothing to change; plugin tasks
  already extend the allowlist through `ElabContext.bench_tasks`
  (`PluginHost::seed_schemas`).

**What.**
1. In `SpicePlugin::register`, add `r.bench_task("extract", …)` implementing
   `PluginBenchTask::run(args, cx)`. Args arrive as POM `Value`s; a waveform
   crosses as `Value::Object` **only in-process** — so `extract` must be
   documented native-tier-only for now (wire tiers can't carry `Object`s;
   serde on `Object` fails loud, which is correct).
2. Start with three measurements, each a pure function over
   `Vec<(f64, f64)>` points: `extract(trace, "rise_time", lo, hi)`,
   `"crossing"`, `"max"`. Downcast the `Value::Object` to
   `piperine_bench::waveform::Waveform` via `Object::as_any`, exactly like
   `$noise` downcasts `NetRef` (`crates/piperine-bench/src/tasks.rs`).
   This requires the spice plugin to depend on `piperine-bench` — acceptable:
   it is a native plugin extending the bench.
3. `.attach`/`.meta` need result objects to carry a metadata map:
   add `metadata: RefCell<HashMap<String, Value>>` to `OpResult`/`Trace`
   (`crates/piperine-bench/src/objects.rs`, `waveform.rs`) with
   `Object::call_method` handlers `attach(key, value)` / `meta(key)`.
   That half is core, not plugin — do it first, then the plugin's `extract`
   can annotate results.

**Rationale.** Measurement logic does not belong in the core (endless list,
vendor-specific); the object-metadata slots do (they're POM-value plumbing).

**Test.** In the spice plugin's tests: run a bench with an RC step response
through the in-process plugin host, `$extract(tr, "rise_time", 0.1, 0.9)`,
assert against the analytic `2.197·RC`.

---

## Part B — Codegen / solver

### B0. Flattener shared-temporary lowering — DONE (2026-07-11)

**Was:** `jit/flatten.rs::subst_scope` inlined every `var` reference with a
full copy of its bound expression. A body that reassigns a var under a guard
(`vd = $limit(…, vd)`) and reuses it many times downstream expanded
**multiplicatively** — the diode reuses `vd` ~30× and the `tBrkdwnV` 25-step
chain compounded it — so the intermediate `PomExpr` tree exceeded RAM before
the emitter's CSE ever ran (a hard OOM that froze the dev machine). This was
pre-existing, independent of the `T?` migration (the sentinel diode OOM'd
identically).

**Fix (shipped): a value/derivative temp tape.** Vars are never inlined:

- `jit/flatten.rs` — each `var` (and each guarded reassignment) becomes one
  entry in `FlatAnalog.temps`; every use is a `__temp(id)` leaf
  (`subst_scope`). Contributions/forces/charge/noise carry only leaves, so
  every expression is linear in body size. Analog-event action values (which
  run against the persistent var bank, not the tape) inline temps on demand
  (`inline_temps`, bounded — event bodies are small).
- `codegen/builder.rs` + `analog_emit.rs` — the Builder holds the value tape
  and a derivative tape; `__temp(id)`/`__dtemp(id)` emit once each, memoized.
- `lower/diff.rs` — `differentiate` maps `__temp(id)` → `__dtemp(id)`; the
  Jacobian builds the per-branch derivative tape `dtemps[k] = d(temps[k])/dV`
  once (`analog.rs::compile_jacobian`/`compile_force_jacobian`), so the
  Jacobian is linear, not exponential (forward-mode AD over a tape).
- `jit/analog.rs` — `collect_limits` scans the tape; `limit_branch` finds the
  junction `V`/`I` branch by **searching** the tape by id with a visited-set
  (`limit_branches_into`), never rebuilding the inlined tree (which would
  re-expand the param-only `tBrkdwnV` chain exponentially — the one subtlety
  that cost a second iteration).

**Result:** the spice `dio` compiles and `$op` converges to the Shockley drop
(0.693 V, 4.31 mA — `DioBias::test_forward_drop` green); the full workspace
(53 targets) and every spice suite stay green; no node budget/guard needed —
the tree is linear by construction. Regression pinned by
`spec_simulation::analog_var_reuse_does_not_explode`.

**Still open:** `bjt`/`mos1` now *compile* (no OOM) but need B5 (multi-junction
`$limit` convergence). A perf follow-up could hoist param-only temps to a
once-per-instance setup bank (ngspice `diotemp`/`dioload` split); correctness
does not require it.

### B1. `transition(expr, td, rise, fall)` operator

**Why.** Behavioral digital-to-analog sources (DAC models, ideal drivers) need
finite-slope transitions; today the operator is recognized in the resolved form
and fails loud at codegen.

**Where.**
- `crates/piperine-codegen/src/jit/flatten.rs` — split like `ddt` is split
  today (see the `charge` handling).
- `crates/piperine-codegen/src/jit/analog.rs` — a runtime-operator row.
- `crates/piperine-codegen/src/device/analog.rs` — the time-stepping state
  machine (this is where `delay`/`slew` already live: copy their structure).

**What.** `transition` is **not** a companion model — it is a runtime operator
like `slew`: at each accepted timepoint the device evaluates the target value,
and the operator's output ramps toward it with the given rise/fall rates,
delayed by `td`. Implementation:
1. In the flattener, treat `transition(x, td, r, f)` exactly like `slew`:
   allocate an operator slot (state: current output, target, switch time),
   substitute the slot's output value into the expression.
2. In `device/analog.rs`, service the slot per accepted step (the same place
   `slew` updates): if `target != output`, move output toward target at
   `rise`/`fall` rate; expose the *slope limit* to the timestep controller the
   same way `slew` does so the ramp is resolved by the integrator.
3. DC/`$op`: `transition` output = its input (steady state). AC: transparent
   (derivative 1), same as `slew`.

**Rationale.** ngspice/Verilog-A define `transition` for piecewise-constant
inputs; modeling it as slew-with-delay on the evaluated input matches the
common analog-behavioral usage and reuses tested machinery. Document the
restriction (input should be piecewise-constant; continuous inputs get
slew-like behavior) in Part V §2.

**Test.** `spec_simulation.rs`: pulse source through `transition(…, 0, 1n, 1n)`
into a resistor; `$tran`; assert the output crosses 50 % one rise-time-half
after the input step, and DC equals the input.

### B2. `laplace_*` / `zi_*` operators

**Why.** Frequency-domain behavioral filters. Recognized, fail-loud.

**Where.** Same three files as B1, plus `crates/piperine-solver` for the extra
MNA states.

**What.** Implement **only `laplace_nd` (numerator/denominator polynomial)
first** — every other `laplace_*` form converts to it algebraically at
lowering time, and `zi_*` stays fail-loud until someone needs it. The
observer-canonical realization: a denominator of order *n* adds *n* internal
MNA unknowns (state variables `x1..xn`) and *n* auxiliary equations
`dxi/dt = …` stamped like any other `ddt` companion; the operator output is a
linear combination of states. Steps:
1. Flattener: an operator slot carrying the normalized coefficient vectors.
2. `CircuitCompiler`/`AnalogInstance::setup` allocates *n* internal nodes per
   `laplace_nd` (the same internal-node mechanism the OSDI seam needs — build
   it once, see E3).
3. Stamps: the state equations are linear and constant-coefficient — stamp the
   companion (transient: backward-Euler/trap on each state; AC: exact
   `jω`-polynomial evaluation; DC: `s = 0` → output = (b0/a0)·input).

**Rationale.** State-space realization is the standard, numerically sane way;
it reuses the existing `ddt` integration path instead of inventing an IIR
history buffer, and it makes AC exact.

**Test.** First-order lowpass `laplace_nd(V(in), [1], [1, 1/(2π·1k)])`:
`$ac` magnitude at 1 kHz = −3 dB ±0.01; `$tran` step response time constant.

### B3. `table(x, xs, ys, mode)` — register, then implement

**Why.** Spec Part V §2 defines it; today it isn't even registered, so it
resolves as unknown function instead of failing loud as unsupported.

**Where.**
- `crates/piperine-codegen/src/lower/pom/analog_ops.rs` — register `"table"`.
- `crates/piperine-codegen/src/lower/symbols.rs` — `Table` symbol exists.
- `crates/piperine-codegen/src/jit/emit.rs` (the emitter file under `jit/`;
  find it via the `CseKey` machinery) — emit the interpolation.
- `crates/piperine-lang/src/eval/` — interpreter evaluation for bench/const
  use.

**What.** Two steps, shippable separately:
1. **Registration (small, do immediately):** add a `TableOp` to
   `AnalogOpRegistry::with_builtins` that lowers to the existing `Table`
   symbol; codegen fails loud (`CodegenError::Unsupported("table")`). This
   converts a confusing "unknown function" into the documented gap.
2. **Implementation:** xs/ys must be elaboration-constant lists (fold them at
   lowering; error if not). Emit a branchless linear-interp chain: clamp x to
   [xs.first, xs.last], then for each segment `select(x < xs[i+1], seg_i, acc)`
   — O(n) straight-line code, CSE-friendly, no loops (Cranelift residuals are
   one block). `mode`: `"linear"` only; other modes fail loud. Derivative for
   the Jacobian: the same select chain over segment slopes (piecewise-constant
   dy/dx) — add a `Table` case to `lower/diff.rs` returning that chain.

**Rationale.** Constant tables cover the real use (device curves); dynamic
tables would need runtime memory in the kernel ABI for marginal value.
Branchless selects keep the single-block invariant of the residual emitter.

**Test.** Diode-ish I(V) table on a 2-point and a 5-point table; `$op` at a
bias inside segment 2 → exact linear value; Jacobian check via convergence.

### B4. Multiple `ac_stim` per contribution

**Why.** `V(p,n) <+ f(V) + ac_stim(m1,p1) + ac_stim(m2,p2)` fails loud today.
Rare but legal (superposed stimuli).

**Where.** `crates/piperine-codegen/src/jit/flatten.rs` (the contribution
splitter that today errors on the second `ac_stim`), `jit/analog.rs` (mag/
phase rows), `device/analog.rs::load_ac`.

**What.** Sum the phasors at flatten time: two stimuli with constant
mag/phase fold into one `(re, im)` pair — `re = Σ mi·cos(pi)`,
`im = Σ mi·sin(pi)`. If mags/phases are expressions, keep a `Vec` of
`(mag, phase)` rows per contribution and sum the complex values in `load_ac`.
The `Vec` shape is the general fix; do that (it subsumes the fold).

**Rationale.** AC is linear; superposition is exact. Changing the row storage
from `Option<(mag, phase)>` to `Vec<(mag, phase)>` is mechanical.

**Test.** Two `ac_stim(1,0)` in one source ⇒ AC response exactly 2× the
single-stim circuit; one with phase π cancels to 0.

### B5. Multi-junction convergence — DONE (2026-07-11, gmin stepping)

**Resolved.** `bjt` (coupled B-E/B-C), `mos1`, and `jfet` all converge to
their operating points; spice `tests/junction.phdl` covers all four junction
devices green. The cure was **gmin stepping** (SPICE homotopy), not more
per-junction limiter tuning:

- `crates/piperine-solver/src/solver/dc.rs` — when plain Newton fails,
  `DcAnalysis::solve` falls back to `solve_gmin_stepping`: a node-to-ground
  conductance (`Context.gmin_extra`) starts at 0.1 S (diagonally dominant,
  trivially convergent) and ramps geometrically to ~0, warm-starting each
  step, with adaptive back-off on any step that stalls; a final solve at
  `gmin_extra = 0` gives the true operating point. `DcSystem::assemble`
  stamps the diagonal conductance on every voltage node.
- Two model/compiler bugs surfaced along the way and were fixed:
  `emit_analog_binary` now treats the `BitXor` pow-carrier (POM has no `Pow`
  variant; `d_math("pow")` emits it) as `pow` (was: "unsupported bitwise" —
  first hit by the BJT `qb` `pow(arg_q, nkf)` Jacobian); and a node/var name
  clash (`mos1`'s depletion-charge `var s` shadowing the source node `s`)
  now resolves to the node — the flattener skips registering a `var` whose
  name is also a node (`jit/flatten.rs::new`), and the model renamed it.

The per-junction pnjlim machinery (`limited_volts`, `limiting_active`,
`update_limits`, vcrit seeding) was already correct from the diode work; it
just wasn't enough on its own for the reverse-biased-collector oscillation,
which is exactly what gmin stepping tames.

**Still open (perf/parity, not convergence):** exact ngspice numerical
parity has not been diff'd against a running ngspice (needs a built
`~/Git/ngspice`); `fetlim`/`DEVlimvds` remain identity (mos1 converges via
gmin stepping without them, but tight ngspice parity may want them).

<details><summary>Original plan (superseded — kept for the fetlim/limvds detail)</summary>

**Why.** The single-junction case (diode) converges; coupled junctions still
blow up. This blocks the bipolar/MOS models in `piperine-spice` — **the
highest-value solver item on this list.**

**Where.**
- `crates/piperine-codegen/src/jit/analog.rs` — `collect_limits` /
  `limit_update` (the per-`$limit` `vold` state slots).
- `crates/piperine-codegen/src/device/analog.rs` — `limited_volts` (the
  limited-Norton linearization) and `Device::limiting_active`.
- `crates/piperine-codegen/src/jit/emit.rs` — `emit_pnjlim`; add
  `emit_fetlim`.
- Reference: `~/Git/ngspice/src/spicelib/devices/bjt/bjtload.c`,
  `mos1/mos1load.c`, and `ngspice/src/spicelib/devices/devsup.c`
  (`DEVpnjlim`, `DEVfetlim`, `DEVlimvds`).

**What.**
1. **Per-junction limited Norton with shared nodes.** Today `limited_volts`
   computes one limited voltage per `$limit` and adjusts `cdeq = cd − gd·vlim`
   assuming the junction voltage is a plain branch of the device. For a BJT,
   V(B,E) and V(B,C) share node B: the Norton correction of each junction must
   be applied to *that junction's* residual rows with *that junction's*
   limited voltage, never a mixture. Concretely: the kernel must evaluate the
   residual/Jacobian **at the vector of limited voltages** (each `$limit` call
   site substitutes its own `vnew`), which it already does (the limiter is
   inlined in the expression) — the bug class is on the *device* side, where
   `limited_volts` must build the full limited voltage vector before the
   single residual call, not limit one junction at a time against unlimited
   others. Audit `limited_volts` for exactly this; fix by two passes:
   (pass 1) compute every junction's `vnew` from the same unlimited solution
   vector; (pass 2) evaluate residual/Jacobian once at the substituted vector.
2. **NaN in bjt:** with (1) in place, seed *every* junction's `vold` at
   MODEINITJCT (`vcrit` per junction, from that junction's saturation
   current — `collect_limits` must key the seed by the `$limit` call's own
   `is`/`vte` arguments, not a device-global). ngspice does exactly this
   (`bjtload.c`, `ICEQmode` init). Verify the exp() guards: pnjlim must be
   applied *before* any `exp(v/vte)` evaluation — i.e. the limiter wraps the
   voltage *inside* the model expression (it does — it's inlined), so NaN can
   only come from an unseeded/incorrectly-seeded `vold`.
3. **`emit_fetlim`:** port `DEVfetlim` (devsup.c) verbatim: limits vgs steps
   around `vto` with the 3.5/2.0 window logic. Same state slot pattern as
   pnjlim. Also port `DEVlimvds` (drain-source damping) — `mos1`'s stall is
   the vds mode-switch oscillation, and `limvds` is ngspice's cure.
4. **Mode-switch damping for mos1:** ngspice recomputes mode (normal/inverse)
   from the *limited* vds. Ensure the PHDL mos1 model routes its mode
   `select` through the limited value (model-side change, in
   `crates/piperine-lang/headers/spice/mos.phdl`, coordinated with this item).

**Rationale.** This is a straight port of ngspice's convergence machinery —
do not innovate here; equality with ngspice iteration behavior is the
correctness standard (piperine-spice rule 2).

**Test.** `spec_simulation.rs`: (a) 2N2222-ish BJT common-emitter bias
converges and matches ngspice operating point to 1e-6 relative; (b) mos1
inverter DC sweep converges at every point; (c) existing diode test stays
green.

</details>

### B6. `@initial` cannot force a branch (`V(p,n) <- ic`)

**Why.** SPICE `.ic`/UIC semantics: capacitors/inductors seeded at t=0.
Today an `@initial` body containing a Force is a loud elaboration error.

**Where.**
- `crates/piperine-codegen/src/jit/flatten.rs` + `device/analog.rs` — collect
  initial forces into data.
- `crates/piperine-solver/src/analysis/` (transient) — the t=0 solve.
- `crates/piperine-solver/src/lib.rs` `Device` trait — one new method.

**What.**
1. At flatten time, an `@initial { V(p,n) <- expr; }` becomes an
   `InitialForce { plus, minus, value_fn }` entry on the compiled module
   (evaluate `expr` with params at device creation — it must be
   instance-constant; error otherwise).
2. New `Device` method `initial_conditions(&self) -> Vec<(NodeId, NodeId, f64)>`
   (default empty).
3. In the transient analysis' initial DC solve (UIC path): for each initial
   condition, stamp a **large-conductance Norton equivalent** across (p,n):
   `G_big·(v − ic)` with `G_big = 1/RIC`, ngspice-style (`CKTsetIC`); release
   the clamp after t=0. This avoids restructuring MNA for hard constraints.
4. `$op` (no UIC) ignores initial forces — matching SPICE `.ic` semantics
   (applied to transient initial solve only). Keep `@initial` variable
   assignments working as today.

**Rationale.** The gmin-style soft clamp is what ngspice does; a hard
constraint row would need branch-current unknowns and buys nothing.

**Test.** Cap with `@initial { V(p,n) <- 3.0; }` discharging through R:
`$tran` first point ≈ 3.0, decay τ = RC.

### B7. `idt` contributes 0 in AC

**Why.** An integrator's small-signal admittance is `1/jω`; today AC through
`idt` is silently wrong (0), violating fail-loud in spirit.

**Where.** `crates/piperine-codegen/src/jit/flatten.rs` (idt slot),
`jit/analog.rs` (AC rows), `device/analog.rs::load_ac`.

**What.** `idt(x)` already has a runtime slot with a known input expression.
For AC: `d(idt(x))/dV = (1/jω)·dx/dV`. Emit the *input's* derivative row
(symbolic diff of `x`, same machinery as the Jacobian) tagged as an
`idt`-kind AC row; in `load_ac`, stamp `dx/dV / (jω)` — i.e. multiply by
`−j/ω`. Structure it exactly like the charge rows (`jω·dQ/dV`), inverted.

**Rationale.** Mirrors the existing `ddt`/charge path; symmetric code, no new
concepts.

**Test.** `V(out) <+ idt(V(in))` driven by `ac_stim`: magnitude at ω is
`1/ω` (±1e-9 rel), phase −90°.

### B8. `Trace.i` (and digital nets) over time — record runtime state per step

**Why.** Two roadmap gaps share one cause: `TransientAnalysisResult` records
node voltages only. Device currents needing runtime state fail loud, and
digital nets aren't readable from `$tran` at all — which also blocks
verifying sequential logic through a bench (the `$op`-rebuild problem).

**Where.**
- `crates/piperine-solver/src/analysis/` (transient result assembly).
- `crates/piperine-solver/src/digital_interface.rs` / topology — digital net
  values per accepted step.
- `crates/piperine-bench/src/waveform.rs`, `objects.rs` — readback.

**What.**
1. Extend `TransientAnalysisResult` with `digital: HashMap<String, Vec<(f64, u8)>>`
   (net name → (time, quad-value) change list — store *changes*, not
   per-step samples; digital is event-driven).
2. The transient loop already services digital events per accepted step;
   record each net's value after event settling. Names come from the same
   map `$op` readback uses.
3. `Trace.v(net)` on a digital net returns a step-waveform (0/1, NaN X/Z),
   piecewise-constant interpolation.
4. For `Trace.i` on stateful devices: after each accepted step the device's
   state bank is *current*; compute branch current on demand is impossible
   post-hoc, so record it forward: add an opt-in probe list — `Trace.i`
   requested nets are declared in the bench *before* `$tran` runs? No —
   simpler and unambiguous: **record every force-branch current** (they're
   already MNA unknowns, free) and keep the fail-loud error for resistive
   currents of stateful devices, with a message telling the user to probe via
   a 0 V source. This matches SPICE practice (current probes are vsources).
5. Update elaboration allowlist/docs; flip the bench spec §11 rows.

**Rationale.** Change-list digital storage is exact and small; the 0 V-source
current-probe convention is 50 years of SPICE precedent and avoids a
record-everything memory blowup.

**Test.** Two-flop shift register across modules, clocked by a pulse source,
`$tran`; assert Q1/Q0 change lists show the sampled pattern (this closes the
"sequential logic can't be verified through a bench" gap). RLC: `Trace.i`
of the driving vsource matches analytic current.

### B9. Fused digital-network JIT — integrate into `run_digital_at`

**Why.** `NetworkComb` (one Cranelift fn per combinational cone) is built and
tested standalone; production still evaluates per-device.

**Where.** `crates/piperine-codegen/src/device/circuit.rs::run_digital_at`,
`crates/piperine-codegen/src/jit/digital/network.rs`, docs
`crates/piperine-codegen/docs/DIGITAL_JIT.md`.

**What.**
1. At `CircuitCompiler` build time, partition the digital device graph:
   maximal cones of purely-combinational devices (no clocked processes, no
   analog interface pins inside the cone) → build one `DigitalNetwork` per
   cone; everything else stays per-device.
2. `run_digital_at` dispatches: cone inputs changed → run the fused fn →
   diff outputs → enqueue events. The `DigitalEventModel` boundary already
   abstracts this — the network is just another event model.
3. Fall back transparently: any device the partitioner can't prove
   combinational stays on the old path. Correctness first; fusing clocked
   members is a *later* item — do not attempt it here.

**Rationale.** The interface was designed for exactly this insertion; the
partition rule ("prove combinational or fall back") keeps it safe.

**Test.** The existing exhaustive examples (`17_ripple_adder_4bit`,
`18_mux4_tree`, `19_multiplier_2x2`, `20_comparator_4bit`) must stay green
*and* a debug counter must show the fused path actually ran (assert cone
count > 0 in a codegen test on the adder).

---

## Part C — Type system

### C1. Optional bundle fields (`model.rbm.get_or(…)`)

**Why.** `T?` params landed; bundle *fields* of optional type can't be read
through `.get_or` in analog bodies (only a direct param receiver folds).
Blocks migrating `piperine-spice` off `1e99` sentinels.

**Where.** `crates/piperine-codegen/src/jit/flatten.rs` (the method-call fold
that today handles `param.is_present()` / `param.get_or(d)`), and
`crates/piperine-lang/src/elab/` where bundle params flatten into per-field
params.

**What.** A bundle param `model : DiodeModel` flattens into per-field
synthetic params (`model.is`, `model.rs`, …). An optional field
`rbm : Real?` flattens into the same parameter-presence pair every optional
param uses (`value` + `given` flag). The fix is receiver resolution: when the
method-call folder sees `Field(Ident("model"), "rbm")` as receiver of
`get_or`/`is_present`, resolve it to the flattened synthetic param name
(`model.rbm`) and reuse the existing param fold. It is a lookup-path
extension, not new semantics.

**Rationale.** The flattened representation already exists; only the
receiver-matching pattern is too narrow.

**Test.** `spec_simulation.rs`: module with `param model : M` where
`bundle M { rbm : Real? = none; }`, body uses `model.rbm.get_or(10.0)`;
instances with and without `.model.rbm` set both solve correctly.

### C2. `From<T>` widening capability (+ literal coercion)

**Why.** The widening table is a hardcoded `matches!` block in
`typecheck.rs`; rules are invisible in the prelude and inextensible.

**Where.** `crates/piperine-lang/src/elab/` typechecker (`typecheck.rs`,
the six-pair `matches!`), `headers/` prelude, `docs/spec/part_i_*.md` §6.1.

**What.**
1. Prelude: `capability From<T> { fn from(v: T) -> Self; }` plus the six
   existing impls as `extern impl From<Boolean> for Quad;` etc. (depends on
   D1 `extern impl`; until that lands, plain `impl` with compiler-recognized
   marker bodies is acceptable but worse — prefer sequencing after D1).
2. Typechecker: replace the `matches!` block with a query into the impl
   registry: `widens(from, to) := impl_exists(From<from>, to)`. Keep a
   compile-time-built cache (HashSet of pairs) so checking stays O(1).
3. Integer-literal coercion (`0`/`1` as Boolean/Quad/Natural) becomes
   `impl FromLiteral<Integer> for Boolean` etc. — same mechanism, separate
   capability so a *literal* rule can't accidentally allow a *variable*
   widening.
4. The elaborated `Design` must still fold the same way — widening inserts
   the same implicit conversion node as today; only the *decision* moves.

**Rationale.** The rule becomes data in the prelude, the checker becomes a
lookup, users see the truth. No behavior change intended — the test is that
nothing else changes.

**Test.** Full suite unchanged; plus one negative test (`Real` ← `Quad`
without an impl still errors) and one prelude-extension test (adding a test
impl makes a previously-failing widening pass).

### C3. Intrinsic capability satisfaction — explicit `impl`s

**Why.** `Real satisfies Add` exists nowhere in source.

**Where.** `headers/` prelude; the operator-desugar pass in
`crates/piperine-lang/src/elab/`.

**What.** Add `extern impl Add for Real`, … for the full primitive×operator
matrix (data-driven: generate the header block once, by hand, from the
desugar pass's table — then make the desugar pass *assert at startup* that
every hardcoded (type, op) pair has a matching prelude impl, so the two can
never drift). Sequencing: after D1 (`extern impl`).

**Rationale.** The runtime dispatch stays hardcoded (fast); the prelude
becomes the visible contract; the startup cross-check makes the table and
the prelude one source of truth with a loud failure mode.

**Test.** Startup cross-check firing = unit test (remove one impl in a
test-only header copy → elaboration error).

### C4. `Iterable<T>` capability

**Why.** `for` hardcodes `Value::List` + `Range`; `Map`/`Set`/user types
can't be iterated.

**Where.** `crates/piperine-lang/src/eval/interp.rs` (the `for` evaluation),
prelude headers.

**What.**
1. Prelude: `capability Iterable<T> { fn iter(self) -> List<T>; }` — note:
   *materializing* iteration (returns a list), not a lazy iterator protocol.
   The interpreter walks lists; lazy `next()` would drag a stateful-object
   protocol into the value layer for zero bench-scale benefit.
2. Interpreter `for`: `Value::List` and `Range` keep their fast paths;
   otherwise resolve `iter` through the impl registry (same dispatch as
   `Host::resolve_method` on tagged records) and iterate the returned list.
3. Builtin impls: `Map<K,V>` → `List<(K,V)>`, `Set<T>` → `List<T>` — do
   these as interpreter-known conversions registered under the capability
   name, not as PHDL bodies.

**Rationale.** Materializing keeps the interpreter simple and the semantics
obvious; bench collections are small.

**Test.** `for (k, v) in my_map` in a bench (pairs with C6) sums correctly;
user record type with an `impl Iterable` body iterates.

### C5. Tuple type resolution

**Why.** `(Real, String)` parses (`ValueType::Tuple`) but resolution/checking
ignore it — annotations are decorative.

**Where.** `crates/piperine-lang/src/elab/` — `resolve_type` and the
typechecker's annotation-vs-value check.

**What.** Add the `ValueType::Tuple(items)` arm everywhere `resolve_type`
recurses (resolve each element) and everywhere assignability is checked
(`Tuple(a) ≤ Tuple(b)` iff same arity and element-wise assignable). Then
`fn foo() -> (Real, Natural)`, `var x : (Real, String) = …`, and
`Vec<(Real, Real)>` all check. Grep for `ValueType::` match sites in `elab/`
to find every arm that needs the case — the compiler's non-exhaustive-match
errors after adding a `#[non_exhaustive]`-style audit are the checklist.

**Rationale.** Pure plumbing; the parse and value layers already agree.

**Test.** `parse_elab.rs`: good annotation passes; arity mismatch and
element-type mismatch produce typed errors naming the position.

### C6. `for (a, b) in …` tuple destructuring

**Why.** Loop bodies index `case.0` — noisy.

**Where.** `crates/piperine-lang/src/parse/` (for-loop pattern),
`eval/interp.rs` (binding).

**What.** Parser: after `for`, accept `(ident, ident, …)` as well as a bare
ident (`ForPattern::Name(String) | Tuple(Vec<String>)` on the AST node).
Interpreter: on `Tuple` pattern, the element must be `Value::Tuple` of the
same arity (else `EvalError::TypeMismatch` naming arities); bind positionally.
No nesting in v1 (`for ((a,b),c)` — reject at parse with "nested patterns not
supported").

**Rationale.** Covers 100 % of current usage (`sweep` result pairs) with a
two-arm enum.

**Test.** Bench iterating `[(1.0, 2.0), (3.0, 4.0)]` destructured; arity
mismatch errors loudly.

### C7. `Value::FnRef` — gate test + typecheck

**Why.** Passing named fns landed (`Value::FnRef`); missing the end-to-end
gate test and the `fn(T) -> R` signature check.

**Where.** `crates/piperine-bench/tests/bench.rs` (gate test),
`crates/piperine-lang/src/elab/` typechecker.

**What.**
1. Gate test: `fn double(x: Real) -> Real { … }  fn apply(f: fn(Real) -> Real,
   v: Real) -> Real { return f(v); }` called from a bench — asserts 10.0.
2. Typecheck: where an argument's declared type is `ValueType::FnPtr(sig)`
   and the argument expression is an identifier resolving to a registered
   callable, compare the callable's declared signature to `sig`
   (arity + element types, return type); mismatch is a typed elaboration
   error. Lambdas: check arity only (param types unknown until C8).

**Rationale.** Finishing an 80 %-landed feature; the check prevents the
interpreter-crash class (wrong arity dispatch).

**Test.** Both of the above plus a negative (passing `fn(Real, Real)` where
`fn(Real)` expected → elaboration error).

### C8. `var` type inference (+ lambda params)

**Why.** `var acc : Real = 0.0;` verbosity in compiled contexts; bench `var`
types are decorative.

**Where.** `crates/piperine-lang/src/elab/` — behavior lowering
(`behavior.rs`, the "type required" error site) and the expression
typechecker (it must already compute expression types to check contributions
— reuse that).

**What.**
1. In compiled contexts: when a `var` has an initializer and no annotation,
   run the expression typechecker on the initializer and adopt its type.
   Error unchanged when there's neither annotation nor initializer.
2. In bench: same inference, and when an annotation *is* present, check it
   against the initializer type (kills "decorative").
3. Lambda parameter inference (second commit): when a lambda literal is the
   argument for a declared `fn(T,…) -> R` parameter, push the declared
   parameter types onto the lambda's params before checking its body. Only
   this call-site-driven form — no Hindley-Milner, no body-driven inference.

**Rationale.** Initializer-driven and signature-driven inference are local,
predictable, and cover the verbosity complaints; anything fancier fights the
one-pass elaborator.

**Test.** `var x = 0.1; x + V(a)` compiles (Real); `var x = [1,2];` infers
`Vec<Natural>`; bench `var y : String = 1.0;` now errors; lambda passed to
`apply` (C7) with unannotated param compiles.

### C9. Discipline nature access by declared name (`Temp(th)` bug)

**Why.** The flattener hardcodes `"V" → Potential, everything else → Flow`,
so `Temp(th)` — a potential — compiles as a flow. **Silently wrong physics;
highest-priority item in Part C.**

**Where.** `crates/piperine-codegen/src/jit/flatten.rs` (the
`nature_kind = match name` block), with the discipline info coming from the
POM `Design.disciplines` (note: currently `#[serde(skip)]` on the wire — the
lowering runs host-side, so in-memory access is fine).

**What.**
1. At flatten time the module's ports/wires know their discipline. Build a
   per-discipline map `access_name → NatureKind` from the declared natures:
   the *capitalized nature name* (`Temp` from `potential temp`) and the
   canonical `V`/`I` aliases for the electrical special case. Where exactly
   the alias rule comes from: Part I §10.1 — access fn is the nature name
   with first letter uppercased; `V`/`I` are the electrical names, not
   built-ins.
2. Replace the `match name` with: look up the accessed net's discipline,
   then the access name in that discipline's map; unknown access name on
   that discipline = loud `CodegenError` naming both.
3. The interpreter side (`eval/`) has the same hardcode risk — audit
   `piperine-lang/src/eval/` for `"V"` matches and route through the same
   POM-derived map (put the map builder on `Design` or the discipline node
   so both consumers share it: one owner).

**Rationale.** The information is already declared in source; the hardcode
predates multi-discipline support. Shared map on the POM = one truth.

**Test.** Thermal RC (`discipline Thermal { potential temp; flow pwr; }`),
`Pwr(th) <+ Temp(th)/rth + cth*ddt(Temp(th))`; `$op` with a power source
gives the right temperature (was wrong/garbage before).

---

## Part D — `extern` declarations

### D1. Elaborator support + `extern impl`

**Why.** Grammar/parser for `extern fn` landed; the elaborator ignores
`is_extern` — the contracts aren't checked or registered. `extern impl`
(needed by C2/C3) isn't parsed yet.

**Where.** `crates/piperine-lang/src/parse/item.rs` (extend to `impl`),
`crates/piperine-lang/src/elab/` (callable registry + impl registry),
`headers/` (prelude migration).

**What.**
1. Elaborator, `extern fn`: register the signature in the callable registry
   as `Callable::Extern` (no body). Calls typecheck against it. If the
   compiler has a builtin with that name (math table, syscall, analog op),
   the extern decl *binds* to it — assert at registration that the builtin
   exists; unknown extern name = elaboration error ("compiler provides no
   body for `foo`"). A source-level body on an `extern fn` = error.
2. Cross-check arities: at startup (debug assertion + one unit test), walk
   the math table (`piperine-math`), analog-op registry, and syscall registry
   and require each entry to have a matching prelude `extern fn` with
   matching arity. This is the living-catalog guarantee.
3. `extern impl`: parser accepts `extern impl Cap for Type { fn …; }` with
   signature-only methods; elaborator records it in the impl registry flagged
   intrinsic. C2/C3 consume this.
4. Prelude migration is incremental: math fns first (25), then syscalls,
   analog operators, events — one commit each, cross-check extended each
   time.

**Rationale.** Bind-to-builtin (not parallel registration) keeps a single
dispatch path; the startup cross-check turns doc-drift into a build failure.

**Test.** Hover/goto in LSP picks up `extern fn sqrt` (E-items); wrong-arity
call to `ddt` is an *elaboration* error (today caught later); removing a
table entry breaks the cross-check test.

---

## Part E — Language server

Ordered by user pain. All in `crates/piperine-lang-server/`.

### E1. Scope-aware name resolution

**Why.** `resolve_at` is global-first-match: hovering `p` in module B can show
module A's port.

**What/Where.** The elaborator already builds per-module name→item maps;
expose them as a query: `Design::resolve_name(module: &str, name: &str) →
Option<PomRef>` on the POM (owner: `pom/design.rs`, next to the existing
accessors). `symbol_index::resolve_at` first determines the enclosing module
from the cursor position (the document's item spans — already available from
parse), then asks the design, falling back to globals (consts, fns,
disciplines). References/rename/highlight (the word-scan versions) then
filter candidate occurrences through the same resolution: an occurrence
counts only if `resolve_at(its position)` returns the same `PomRef`.

**Rationale.** Reuses elaboration results instead of building a second
resolver; the occurrence-filter trick upgrades three features (references,
rename, highlight) with one primitive.

**Test.** Two modules with a port `p` each: hover in B shows B's port; rename
in B doesn't touch A; a comment containing `p` is not renamed.

### E2. Project-unit elaboration

**Why.** Documents elaborate per-file; cross-file goto/rename don't work in
multi-file projects.

**What/Where.** `ServerState` gains `projects: HashMap<PathBuf /*root*/,
ProjectState>` where `ProjectState` caches the project `SourceMap` + last
good `Design`. `DocumentState::analyze` on a file under a `Piperine.toml`
root elaborates the *project* (all files via the existing
`ProjectContext::discover`), and diagnostics fan out per-file using each
error's span→file mapping from the `SourceMap`. Debounce: re-elaborate at
most once per keystroke-burst (the existing analyze scheduling already
debounces per-document; key it per-project instead).

**Test.** Two-file fixture project: goto-definition from file A's instance to
file B's module; error in B shows squiggle in B even when A is the open file.

### E3. Protocol-level tests

**What/Where.** New `tests/protocol.rs` using
`lsp_server::Connection::memory()`: initialize → didOpen → hover →
completion → shutdown, asserting JSON responses. One happy-path test per
handler family is enough; the point is catching id/response-shape breakage
the helper tests can't see.

### E4. Error-accumulating elaboration

**Why.** First `ElabError` stops analysis — the editor shows one error at a
time.

**What/Where.** In `crates/piperine-lang/src/elab/mod.rs`: elaboration
already visits items in sequence; convert the per-item `?` into
`errors.push(e); continue` at the item granularity (module-level items and
per-behavior lowering). Return `Result<Design, Vec<ElabError>>` behind a new
entry point `parse_and_elaborate_all_errors` used by the LSP only —
the CLI keeps first-error (miette single-report) semantics until someone
asks. Do **not** try to recover *inside* an item body; item-level
granularity is the 90 % win with none of the poisoned-state risk.

**Test.** Fixture with errors in two different modules → both diagnostics
appear.

### E5. Attribute-schema IDE support

**What/Where.** The schema registry (`elab/registry/schemas.rs`) is in the
`ElabContext`; thread the populated `SchemaRegistry` into the analysis result
(`DocumentState`), then: completion on `@` lists schema names; hover on
`@name` prints its `AttrField`s; unknown field / wrong type / missing
required already produce elaboration errors — verify they carry spans and
fan out (E4). Goto-definition on plugin-registered schemas: none (no source
location) — return the manifest path instead.

**Test.** Completion after `@` contains `device`; hover shows field list.

---

## Part F — Spec divergences

### F1. E2021 `PrivateItem` never raised

**What/Where.** `crates/piperine-lang/src/pom/` resolver (`resolve.rs`):
during `use` resolution, when privacy filtering drops a name, record it in a
`filtered: HashMap<String, /*origin pkg*/ String>` alongside the scope. At
E2002/E2003 ("not in scope") raise time, check `filtered` first and raise
E2021 ("`foo` exists in `pkg` but is private") instead.

**Test.** Fixture: private fn accessed cross-package → E2021 with both names.

### F2. Selector axes `driver::` / `load::` / `parent::` / `ancestor::`

**What/Where.** `crates/piperine-lang/src/pom/selector/eval.rs` +
POM additions.
1. `parent::`/`ancestor::` (do first — cheap): the walker that evaluates
   selectors traverses `Design → Module → Instance`; it always knows the
   path. Keep an explicit parent stack in the evaluator (no POM change) and
   implement both axes over it.
2. `driver::`/`load::` need per-net direction info: build it on demand
   (lazily, cached per `Design` generation) from instance ports — a port
   with direction `output`/`inout` on a net is a driver; `input` a load.
   Owner: a `Connectivity` helper struct in `pom/selector/` built from the
   design, not stored in the POM (derivable data stays derived).

**Test.** Selector tests in `pom/selector/tests`: 4 new axis cases on the
existing fixture design.

### F3. `pub` headers + drop the `piperine::` privacy exemption

**What/Where.** Mechanical: add `pub` to every declaration in `headers/*.phdl`
(frozen corpus — this is the sanctioned kind of edit: sweep, don't
restructure), then delete the `use piperine::…` special case in `resolve.rs`.

**Test.** Full suite green (any missed `pub` fails loudly); one test that a
non-`pub` item in a user package is still filtered.

### F4. Keyword tokenization (lexer-level reservation)

Deliberately **wontfix for now** — documented as current design (Part I
§4.2). Revisit only if a parser bug traces back to identifier/keyword
ambiguity. Keep this note here so the audit doesn't re-open it.

---

## Part G — Plugin system leftovers

### G1. Artifact distribution (prebuilt binaries from git releases)

**Why.** A git-sourced plugin currently must contain a prebuilt `entry`
artifact in-tree or be built manually; the host **never builds plugin
sources** (security invariant).

**Where.** `crates/piperine-project/src/resolver.rs` (`resolve_plugins`),
manifest schema (`crates/piperine-plugin/src/manifest.rs`), spec §5.2.

**What.**
1. Manifest gains an `[artifacts]` table: `base_url` (or "github-releases")
   and per-target entries: `x86_64-unknown-linux-gnu = "libfoo-x86_64.so"`.
2. After git checkout, if `entry` doesn't exist: construct the release-asset
   URL from the checked-out tag (`release/vX.Y.Z` → tag asset), download to
   `target/plugins/<name>/artifacts/<file>`, and verify: the manifest **must**
   carry `sha256-<target>` for each artifact; mismatch = P0007
   `HashMismatch`, no hash declared = refuse to download (loud). TOFU then
   applies to the downloaded artifact exactly as to a local one.
3. Downloader: shell out to `curl`/`git` like the resolver already shells to
   git — no new HTTP dependency.
4. Offline mode: if the artifact is already present and hash-valid, no
   network touch.

**Rationale.** Hash-pinned by the manifest (which is itself hash-pinned by
the lockfile) keeps the no-build security story airtight; per-target tables
match how release CI actually uploads.

**Test.** `piperine-plugin/tests/`: fake "release" as a local file URL;
resolve downloads, verifies, loads; corrupted file → P0007.

### G2. Wire-tier scripts (capability-gated fs for WASM/process guests)

**Why.** Declaring a script from a WASM/process guest is a load error today;
the spice transcriber wants to eventually run sandboxed.

**Where.** `crates/piperine-plugin/src/backend/wasm.rs` (host imports),
`process.rs` (RPC methods), `pom/wire.rs` (protocol), `capability.rs`
(the enforcement — reuse `HostCtx::fs_read/fs_write` verbatim).

**What.** Add two host-side functions exposed to guests:
`host_fs_read(path) → bytes` / `host_fs_write(path, bytes)`, both routed
through the plugin's `HostCtx` (same glob/`..` checks → P0002). WASM: two
imports in the linker (JSON-free: raw bytes via linear memory, packed i64
returns). Process: two new RPC methods the *guest* sends to the host
(reverse-direction request — the host's read loop must handle guest-initiated
requests while waiting for the script result; frame with the same
`RpcRequest` shape and an `id` namespace flag). Then delete the
"scripts on wire tiers" load error and route `run_script` through the
transport.

**Rationale.** Reusing `HostCtx` means the permission model is identical
across tiers — the manifest is the only authority.

**Test.** `wasm_smoke`/`process_smoke`: guest script reads an allowed file,
writes an allowed output; a `..` path from the guest → P0002.

### G3. OSDI internal-node seam (`DeviceProvider` netlist handle)

**Why.** OSDI models allocate internal MNA nodes at setup;
`PluginDeviceSpec` only carries already-connected terminals, so
`@device(plugin = "osdi")` fails loud.

**Where.** `crates/piperine-codegen/src/device/provider.rs` (the spec/trait),
`device/circuit.rs::add_plugin_instance`, `~/Git/piperine-osdi/src/device.rs`
(consume it).

**What.**
1. Extend the factory signature: `instantiate(spec, netlist: &mut dyn
   NetlistHandle)` where `NetlistHandle { fn internal_node(&mut self, label:
   &str) -> AnalogReference; }` — allocates a fresh MNA unknown scoped to the
   instance (name: `<instance>.<label>`, visible in `$op` readback like any
   node).
2. `CircuitCompiler` implements `NetlistHandle` over its node table (it
   already allocates nodes for wires; same counter).
3. `piperine-osdi`'s factory calls `internal_node` for each OSDI-declared
   internal node and wires its stamps accordingly; delete its fail-loud
   stub. Also unblocks B2 (`laplace_nd` states — same handle).

**Rationale.** A callback handle keeps allocation inside the compiler (single
node-numbering owner) while letting devices declare needs at setup.

**Test.** In the osdi repo: a VA resistor with an internal node
(`resistor.va` variant) instantiated through `@device` solves; node shows up
namespaced in results.

### G4. `HookInput.solve` for swept analyses

**Why.** `after_solve` gives plugins node voltages for `$op` only; `$tran`/
`$ac`/`$noise` deliver just the analysis kind — a logger/checker plugin can't
see sweep data.

**Where.** `crates/piperine-plugin/src/view.rs` (`SolveResultView`),
`pom/wire.rs::Solve`, `crates/piperine-bench/src/session.rs` (fire sites).

**What.** Extend `Solve`/`SolveResultView` with
`sweep: Vec<(f64, Vec<(String, f64)>)>` (time/freq point → net values),
capped: fire hooks with at most N=1000 points (uniform decimation, count
documented) — hooks are observers, not data sinks; full data belongs to
result objects. `$noise` carries the PSD sweep in the same shape.

**Test.** `phase3.rs`-style: plugin records `after_solve` for `$tran`,
asserts point count > 0 and matches decimation rule.

---

## Suggested execution order

Dependencies + value, top first:

1. **C9** (nature-access bug — silent wrong physics, small fix)
2. **B5** (multi-junction `$limit` — unblocks piperine-spice bjt/mos, the
   active workstream)
3. **B8** (transient digital + current recording — unblocks sequential-logic
   verification and several bench gaps)
4. **B3.1** (register `table` fail-loud — 10 lines), then B3.2, B4, B7
5. **A1** (uniform API — surface work, high leverage for users)
6. **G3** (internal-node seam — unblocks OSDI *and* B2)
7. **C1** (optional bundle fields — unblocks spice model cleanup), C5, C6, C7
8. **D1**, then **C2/C3** (extern → capability visibility)
9. **B6**, **B1**, **B2** (initial conditions, transition, laplace)
10. **B9** (digital fusion integration)
11. **E1–E5** (LSP track, independent of everything above)
12. **F1–F3** (spec-divergence cleanups, anytime)
13. **G1/G2/G4**, **A2**, **C4/C8** (as demanded)

# ROADMAP.md — Piperine V1 and beyond

Rewritten 2026-07-18 (solver-gaps audit merged in — `SOLVER_GAPS.md` is gone).
Everything delivered before this date was purged (git history + `.specs/` keep
the record; the big solver deliveries — Element ABI, Net naming, TR-BDF2 + PI
controller, LTE stepping, gmin/source stepping, current-residual convergence,
`$limit`/pnjlim, flux companions, `@initial` seed, live params — are
summarized in `CLAUDE.md` and `.specs/STATE.md`). Convention unchanged:
**fail loud** — what the toolchain cannot do is a named error, never a silent
no-op.

Cross-validation harness: root `tests/ngspice_validation.rs`
(+`tests/ngspice/`) — `cargo test -p piperine ngspice` after any solver
change.

---

## North star

Piperine is a complete HDL-centric design toolchain for the **low/medium-level
designer**: hobbyists, independent professionals, and small teams building real
products without access to Cadence-class tooling. The gap we fill is
*integration*: one language and one host that cover design entry, SPICE-class
simulation, mixed-signal, live/interactive simulation, optimization (design
centering), and — through plugins — schematic generation, PCB export, and
digital flows (Yosys, OpenROAD/OpenFASoC). ICs are not the current target, but
the door stays open: the same plugin surface that will drive OpenROAD later.

The **bench is Python** (decision 2026-07-16/17, bench-removal): PHDL describes
circuits; everything procedural — verification, sweeps, optimization,
dashboards — lives in the Python host. Future language-level optimization
support builds on that host, not on a revived in-language bench.

---

## V1 — definition of done

Six pillars. V1 ships when all six are green.

| # | Pillar | One-line bar |
|---|--------|--------------|
| P1 | **Solver complete** | Every analysis a working SPICE user expects, plus PSS; engine gaps closed or explicitly documented as post-V1 |
| P2 | **Low-level device ABI** | External compiled devices load via `libloading` and bind with a PHDL `@device` declaration — OSDI is the first client |
| P3 | **Python library polished** | `import piperine` is the single host: benches, validation, plugins scripting; documented, docstringed, stub-complete |
| P4 | **Language server 100%** | Scope-aware resolution, project-wide navigation, attribute-schema IDE support, protocol-level tests |
| P5 | **Plugin interface simplified** | One clear extension story (attributes + devices + hooks + scripts); native + Python backends only; writing a plugin is a documented afternoon task |
| P6 | **Optimizer** | Design-centering-capable optimization loop on the live-params engine; shape under study — PSS and `.sens` land first as its feeders |

### Architecture decisions

- **MD-20 (locked, user 2026-07-18): `piperine-api` crate.** A dedicated
  `crates/piperine-api`, pure Rust: host API (session/results/waveform/hooks)
  + ABI contracts (device/plugin traits). `piperine-python` becomes a thin
  binding layer over it. The root `piperine` package is **only the CLI
  export** — the `piperine` command line — nothing library-shaped lives in
  root `src/`. Supersedes MD-19's root-as-lib. Dependency flow:
  `api → {lang, codegen, solver}`; `python → api`; `root(bin/cli) →
  {python, api, project}` — no cycle.
- **MD-21 (locked, user 2026-07-18): plugin backends = native + Python.**
  WASM (wasmtime) and process JSON-RPC tiers are removed. Native dlopen stays
  (trusted, fast — same mechanism as the P2 `libloading` device path). Python
  plugins run through the already-existing embedded-host isolation (clean,
  same surface as benches). Requirement: **expose the lifecycle registry to
  Python** so a plugin self-registers (schemas, hooks, scripts, devices)
  transparently on load.
- **Optimizer shape — open study (user).** Not a V1 blocker to decide now.
  Design centering is the target; library-first on `LiveSession` vs
  language-baked `@optimize` stays a To-Do design item under P6. PSS and
  `.sens` are needed regardless and land in P1.

---

## P1 — Solver complete

The merged open-gaps audit (ngspice-46 vs the native solver). Status:
**PARTIAL** (works in some cases) / **MISSING** (absent).

### Analyses

- [ ] **`.dc` sweep — MISSING as a native analysis.** Host-level compile-once
      restamp sweeps (MD-18) cover param sweeps; confirm they cover nested
      sweeps and *source* sweeps, or add the solver-side loop.
- [ ] **`.sens` (DC/AC sensitivity) — MISSING.** Reuse the symbolic-diff
      infrastructure. Medium value alone; high value as the P6 optimizer
      feeder.
- [ ] **PSS (periodic steady state) — MISSING.** Shooting method over the
      transient engine. Required for switching converters; feeds P6.
- [ ] `.four` — Python host post-processing (numpy FFT on `Waveform`), not a
      solver analysis (tracked in P3).
- [ ] `.pz`, `.disto`, `.sp` — MISSING, niche, post-V1.

### Transient

TR-BDF2 core done and active. Breakpoints done — unified table (TRB-11),
sources declare edges via `Element::next_breakpoints`, integrator lands
exactly on them. Print-grid interpolation reclassified as a host feature
(P3). Remaining:

- [ ] **Enforced UIC hold — PARTIAL.** `@initial` seeds t=0; ngspice UIC also
      *holds* the node through the first solve via a large-conductance clamp
      released after t=0.
- [ ] **Inductor flux TR-stage dual — PARTIAL.** The TR stage uses the
      pure-derivative form; previous-voltage tracking is the follow-up (no
      known regression).
- [ ] Remove vestigial `IntegrationMethod` enum (+ `suggest_transient_step`'s
      `method` param). TR-BDF2 is the sole scheme; ~34 references linger.

### Convergence

`gshunt` done (`Tolerances::gshunt`, user-raisable diagonal stamp).
Remaining:

- [ ] **`fetlim`/`DEVlimvds` — PARTIAL.** Identity today
      (`analog_emit.rs`: `"fetlim" => vnew`); MOS converges via gmin stepping
      without them. May matter for exact ngspice parity.

### Engine operator gaps (codegen, all fail loud)

- [ ] `table(x, xs, ys, mode)` — **not registered at all** (resolves as
      unknown fn, never reaches the fail-loud path). Register, then implement
      1-D interpolation.
- [ ] `transition`, `laplace_*`, `zi_*` — recognized in the resolved form; no
      companion models.
- [ ] `idt` AC `1/jω` admittance — contributes 0 in AC.
- [ ] Multiple `ac_stim` per contribution.
- [ ] `@initial` cannot force a branch (event bodies reject Force).
- [ ] `Trace.i` over time on devices reading runtime state/vars — per-step
      banks not recorded in `TransientAnalysisResult`.

### Digital

- [ ] **Fused combinational-network JIT — BUILT, not integrated.**
      `NetworkComb`/`DigitalNetwork` tested standalone; wire into
      `circuit.rs::run_digital_at` (cone detection, per-device fallback for
      clocked/analog members), then fuse clocked members. See
      `piperine-codegen/docs/DIGITAL_JIT.md`.

### SPICE model completeness ("everything I can do in spice, I can do here")

Present and ngspice-validated (live golden/sweep cases, zero ignores):
passives, sources, controlled, switches, diode, BJT, JFET, MOS level 1 —
the old MOS1 1.5×/JFET 15 mV discrepancies were fixed 2026-07-16
(series-impedance forces). Missing:

- [ ] MOS levels 2/3 (level 1 exists).
- [ ] Transmission lines (`tline`, lossy, `urc`).
- [ ] Combined transformer block (`ind`+`mut` as one device — the mutual-flux
      engine supports it; separate devices would double-force one branch).
- [ ] Migrate models off sentinel `$param_given` encodings onto `T?`
      optionals.
- [ ] BSIM-class models arrive via OSDI (P2), not hand-ported PHDL.

### Performance

Done: device bypass (per-variable-threshold stamp cache, suppressed while a
limiter clamps), matrix reuse (symbolic LU reused for the whole run),
transient predictor (CP-16 first-order Newton seed). Remaining:

- [ ] **Temperature sweep — PARTIAL.** Models read `temp`/`dtemp`; confirm
      global `.temp` + `tnom` rescaling flows uniformly (analysis-level sweep
      is host-side).

### Minor refactor leftovers

- [ ] Split `digital/scheduler.rs` into topology/state/scheduler modules.
- [ ] `DcAnalysisResult::as_iv(&Netlist)` — analysis types shouldn't take
      `Netlist`; move or re-sign when the surface finalizes.
- [ ] Shared `Integrator` for noise trapezoid + future `.four`.
- [ ] `SignalBridge` extraction from
      `CircuitInstance::accept_and_run_digital` (three jobs in one method).
- [ ] `Context::default` must not `init_global`; `Solver::build` owns it.

### P1 priority order

1. `.sens` — optimizer feeder.
2. PSS.
3. Remaining models (MOS 2/3, tline/urc, transformer block).
4. Everything else on demand.

---

## P2 — Low-level device ABI (`libloading` + PHDL declaration)

The plugin device path exists (native backend, `@device(plugin=…)`,
`DeviceProvider`). OSDI/ngspice are used as a **checklist for integration
maturity, not as the native ABI** — the native contract stays
mixed-signal-first; OSDI wrappers are one client.

**V1 blockers**

- [ ] **Internal-unknown allocation — MISSING, the P2 blocker.** External
      models need auxiliary nodes/branches allocated pre-finalization. Blocks
      the `@device(plugin = "osdi", …)` PHDL seam (factory fails loud today;
      the `piperine_osdi` Rust API works meanwhile).
- [ ] **Model/instance separation — MISSING.** `ModelHandle` (shared card) vs
      `ElementInstance` (terminals, instance params, state); gives sweeps a
      clean rebuild rule.
- [ ] **Explicit lifecycle — MISSING.** Ordered hooks: model setup → instance
      setup → temperature preprocess → load/evaluate → accept/commit →
      rollback → destroy. One chart per analysis.
- [ ] Artifact distribution — prebuilt plugin binaries per target triple from
      git releases (today the artifact must pre-exist).

**Element ABI maturity checklist (schedule with the first client that needs
each)**

- [ ] **Commit/rollback for all mixed-signal state.** Rejected timesteps must
      restore every stateful participant (A2D crossings, D2A latches, co-sim
      state), not only the digital net array. Hard requirement for the MCU
      co-sim plugin (gallery #1).
- [ ] **Unified event model — PARTIAL.** The unified *breakpoint* table
      (TRB-11, `Element::next_breakpoints`) landed; full unification of
      digital events, analog crossings, timers, and `$bound_step` hints under
      one queue (kind, target, time, priority, source, rollback behavior) is
      still open.
- [ ] Richer terminal descriptors — domain, direction, required/optional,
      sign convention, external/internal/auxiliary.
- [ ] Opvar catalog — declared names/types/units/owner for `gm`, `vbe`,
      register state; uniform query path.
- [ ] Noise metadata — per-source names/types/terminal pairs; per-source
      contribution reporting (today total PSD only).
- [ ] Temperature protocol — nominal/instance/delta separation; declare
      whether a change means recompute constants, restamp, or rebuild.
- [ ] Parameter invalidation rules — partially landed
      (`ParamDescriptor::invalidation`); wire sweeps/optimizer to honor them.
- [ ] Formal limiting API — proposed/limited values, limiter name, active
      state, reason (today `limiting_active` bool).
- [ ] Jacobian/stamp capability declaration — analytic vs numeric vs missing;
      validation error for analyses that need what's absent.
- [ ] Save/probe selection — devices declare observables + cost; record only
      what the host asked.
- [ ] `NewtonStrategy`/`StepperStrategy` — fold Newton damping/limiting and
      transient step rejection into the `ConvergencePlan` composition
      (homotopy half is done).
- [ ] Introspect leftovers: model descriptor (type id/version), real
      opvar/terminal catalogs from the kernel (indices exist, names don't).

---

## P3 — Python library polished

Facade is docstringed and parity-tested (bench-removal). Remaining:

- [ ] `piperine.plot(waveform, ...)` convenience (matplotlib wrapper).
- [ ] `.four`-style post-processing helpers (`waveform.fft()`, etc.).
- [ ] `waveform.resample(grid)` — `.tran tstep`-style print-grid
      interpolation (decision 2026-07-18: host feature, not solver;
      `Waveform.at` already interpolates point queries).
- [ ] `extract` / `.attach` / `.meta`-class helpers as Python host-API
      functions (the plugin bench-task surface died with the bench).
- [ ] Ergonomics pass driven by real bench-writing (error messages, numpy
      seams, keyword defaults).
- [ ] `HookInput.solve` payloads for swept analyses (tran/ac/noise hand hooks
      the analysis name only; op carries node voltages).
- [ ] Packaging/PyPI: post-V1, but keep the module layout PyPI-shaped.

Post-V1 interactivity (oscilloscope, dashboards, sliders driving
`LiveSession.set`) builds here — see gallery.

---

## P4 — Language server 100%

- [ ] True scope-aware name resolution (elaborator name→id maps exposed as a
      query; today first-match global lookup).
- [ ] Resolver-driven references/rename/highlight (today word-occurrence
      scans; comments/strings match).
- [ ] Project-unit elaboration (`ServerState.projects`), cross-file
      goto/rename, per-file diagnostic fan-out.
- [ ] Error-accumulating elaboration (today first `ElabError` stops analysis
      — one error at a time in the editor).
- [ ] Attribute-schema IDE support: completion of `@schema` names, in-editor
      argument validation, hover→schema fields, goto→`@attribute`
      declaration, outline entries.
- [ ] Protocol-level tests over `Connection::memory()` (init → didOpen →
      hover/completion round-trips).

---

## P5 — Plugin interface simplified

Part VI is implemented (manifest, TOFU, attr schemas, `@device`, hooks,
scripts). V1 is **reduction and polish** under MD-21:

- [ ] Remove the WASM (wasmtime) and process JSON-RPC backends +
      `piperine-plugin-wasm`; native dlopen stays.
- [ ] **Python plugin tier**: a `.py` plugin loaded through the embedded host
      (same isolation as benches), with the **lifecycle registry exposed to
      Python** — self-registration of attribute schemas, hooks, scripts, and
      devices on load.
- [ ] One "write a plugin" document with a worked example per extension kind
      (attribute schema, device, hook, script) × per tier (native, Python).

---

## P6 — Optimizer

Target use case: **design centering** (maximize yield over process/tolerance
spread). Foundations in place: compile-once restamp sweeps (MD-18),
`LiveSession` (`set`/`schedule_set`/rebuilds); `.sens` + PSS land in P1.

- [ ] **To-Do design (user studying):** algorithm family (worst-case distance
      vs Monte-Carlo yield vs ellipsoidal) and shape (Python library vs
      language-baked `@optimize`). No decision forced now.
- [ ] V1 deliverable once the study closes: an optimization loop a user runs
      on a real circuit (params in, spec functions out, centered design back)
      with docs and an example.

---

## Post-V1 — plugin gallery (priority order sketch)

1. **MCU co-simulation** — inject event-driven digital devices simulating
   AVR/ESP32-class MCUs (engines: Renode and/or Wokwi cores — possibly both,
   per target family); rides the P2 device ABI + commit/rollback.
2. **Yosys bridge** — translate digital PHDL to Yosys for synthesis +
   open-source programmer flows.
3. **Python interactivity** — digital oscilloscope, dashboards with
   buttons/sliders bound to `LiveSession` params, general ergonomics.
4. **Schematic generation** — `@schematic(...)` attributes → rendered
   schematics from the POM (adoption driver).
5. **OpenROAD / OpenFASoC integration** — design params declared in-language
   via attributes; manage the flow from the HDL.
6. **Richer SPICE interop** — `@spice(symbol = "N", ...)` custom attributes.
7. **PCB export** — `@socket(socket = "DIP", ...)`-style attributes feeding a
   PCB generator/exporter.

---

## Language backlog (schedule on demand — none blocks V1)

Condensed; full design sketches in git history (`ROADMAP.md` pre-2026-07-18).

- **Capabilities for implicit rules**: `From<T>` widening (replace the
  hardcoded typecheck table), intrinsic `impl Add for Real`-style prelude
  visibility, `Iterable<T>` for `for`, `FromLiteral` coercions.
- **`extern` declarations**: parser done for `fn`; elaborator registration,
  prelude migration of math/operators/syscalls/events, `extern impl`, LSP
  first-class symbols. Fixes discipline-nature access too (`Temp(th)` is
  currently mis-lowered as Flow — the one *correctness* item in this list;
  promote if thermal disciplines get real use).
- **Type system**: tuple-type resolution/checking, `fn`-reference gate test +
  typecheck, `var` type inference (+ lambda param inference), `for (a, b)`
  tuple destructuring, bundle-literal field defaults at analog call sites.
- **Host addressability**: net/instance arrays from hosts (`tap[2]`,
  `bank[0]`; `wire x : T[N]` collapses today), leaf-top empty circuits.
- **Spec divergences** (2026-07-07 audit): E2021 `PrivateItem` never raised;
  selector axes `driver::`/`load::`/`parent::`/`ancestor::` fail loud; stdlib
  `pub` exemption (add `pub` to headers, drop the resolver exemption);
  keyword reservation is parser-level (documented, low priority).

---

## Out of agent scope (user-owned)

VS Code extension productization, marketplace packaging, release/versioning —
`editors/vscode/`.

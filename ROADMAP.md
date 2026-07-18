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

- **MD-20 (locked + amended, user 2026-07-18; DONE — feature `api-crate`,
  Verifier PASS): `piperine-api` crate.** `crates/piperine-api` (pure Rust)
  is the library face; the root `piperine` crate is a thin re-export shell
  (`pub use piperine_api::*`, bin stays in `piperine-cli`). ABI-contract
  consolidation deferred to P2/P5.
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

The merged open-gaps audit (ngspice-46 vs the native solver). **CLOSED
2026-07-18** (feature `p1-solver-complete`): every checkbox below is done or
moved to the named backlog table at the end of the section (`urc` blocked on
`codegen-parametric-devices`; `laplace_*`/`zi_*` stay fail-loud).

### Analyses

- [x] **`.dc` sweep — CLOSED 2026-07-18 at the host level** (T1,
      `tests/dc_host_proof.rs`): nested two-param and source sweeps restamp
      one compilation with exact equality vs fresh builds; no solver-side
      analysis needed.
- [x] **`.sens` (DC sensitivity) — DONE 2026-07-18** (p1-solver-complete
      T3/T4): central-difference over the restamp path, uniform surface on
      both hosts (`run_sens` / `module.sens`, MD-22). AC sensitivity and the
      exact-symbolic direct method logged as upgrades.
- [x] **PSS (periodic steady state) — DONE 2026-07-18** (T5/T6): single
      shooting over transient re-entry, damped Newton, 2nd-period
      anti-false-fixed-point guard, digital k·T diagnostic,
      `estimated_settle_time` from the monodromy eigenvalue; uniform hosts
      (`run_pss` / `module.pss`, MD-22) validated on a full-wave rectifier.
- [ ] `.four` — Python host post-processing (numpy FFT on `Waveform`), not a
      solver analysis (tracked in P3).
- [ ] `.pz`, `.disto`, `.sp` — MISSING, niche, post-V1.

### Transient

TR-BDF2 core done and active. Breakpoints done — unified table (TRB-11),
sources declare edges via `Element::next_breakpoints`, integrator lands
exactly on them. Print-grid interpolation reclassified as a host feature
(P3). Remaining:

- [x] **Enforced UIC hold — DONE 2026-07-18** (T11, `b9f47af`): `@initial`
      branch force compiles into the t=0 IC path and the large-conductance
      UIC clamp (ngspice CKTsetIC) releases after the first accepted step;
      pre-charged cap discharge matches `5·e^(−t/RC)`.
- [x] **Inductor flux TR-stage dual — DONE** (fix `d400973`, regression
      proof `f76b4db`): the TR stage subtracts the previous branch voltage
      `V_n` once per flux-carrying branch; RL closed-form (LIVE-07) and the
      coupled-LC energy-transfer regression pin the trajectory.
- [x] **`IntegrationMethod` removed — DONE 2026-07-18** (`1d7e605`): enum,
      dead `TruncationError` trait and `Tolerances.integration` deleted;
      `suggest_transient_step` lost its `method` param. TR-BDF2 is the sole
      scheme.

### Convergence

`gshunt` done (`Tolerances::gshunt`, user-raisable diagonal stamp).
Remaining:

- [x] **`fetlim`/`limvds` — DONE 2026-07-18** (`81f36af`): ngspice
      `DEVfetlim`/`DEVlimvds` ported as branchless select IR on the pnjlim
      slot machinery, unit-tested value-for-value against the C reference
      across every reachable branch; MOS goldens stay green live.

### Engine operator gaps (codegen, all fail loud)

- [x] `table(x, xs, ys)` — DONE (T7, `fd2f83e`): 1-D linear interpolation
      with end clamp, segment-slope Jacobian, loud on non-monotonic axes.
- [x] `transition` — DONE (T8, `c66b2c7`): runtime-operator state bank,
      ramp breakpoints, rejected-step commit/rollback.
- [ ] `laplace_*`, `zi_*` — **backlog (language)**: stay fail-loud (user
      2026-07-18).
- [x] `idt` AC `1/jω` admittance — DONE (T9, `6dedca1`): integrator shows
      −20 dB/dec and −90° across 4 decades.
- [x] Multiple `ac_stim` per contribution — DONE (T10, `660af1c`): phasor
      sum, superposition-proven.
- [x] `@initial` branch force — DONE (T11, `b9f47af`; see UIC hold above).
- [x] `Trace.i` on state-reading devices — DONE 2026-07-18 (`e8f1ff4`):
      opt-in `record_device_state` records per-step runtime banks; off, the
      read stays a loud error.

### Digital

- [x] **Fused combinational-network JIT — ACTIVE** (T12, `4272f61`):
      pure-comb cones evaluate through `DigitalNetwork` (one fused call),
      per-device fallback for clocked/analog-sampling members; bit-identical
      to the per-device path on every digital suite.
- [ ] Clocked-member fusing — **backlog**: the comb integration left the
      scheduler seam clean only for combinational cones; clocked fusing
      touches NBA semantics (logged follow-up, spec assumption 2026-07-18).

### SPICE model completeness ("everything I can do in spice, I can do here")

Present and ngspice-validated (live golden/sweep cases, zero ignores):
passives, sources, controlled, switches, diode, BJT, JFET, MOS levels 1/2/3,
lossless tline, xfmr — the old MOS1 1.5×/JFET 15 mV discrepancies were fixed
2026-07-16 (series-impedance forces). Missing:

- [x] MOS levels 2/3 — DONE (T13/T14, `3c76261`, `c9dcd2a`): ngspice
      goldens per region, live.
- [x] Lossless transmission line — DONE (T15, `6bfc50f`): Branin model over
      the `delay` runtime operator; matched/open termination cases green.
- [ ] `urc` lumped RC line — **BLOCKED on codegen** (T16): parametric
      devices need hierarchy flattening / const-args-into-behavior /
      array-node expansion — tracked as the `codegen-parametric-devices`
      feature. LTRA (lossy tline, full convolution) — **backlog**: urc
      covers the practical lossy case.
- [x] Combined transformer block — DONE (T17, `678dcfe`): `xfmr(l1, l2, k)`
      over the mutual-flux engine; AC ratio and coupled-LC energy transfer
      validated.
- [x] Stdlib off sentinel params — DONE (T18, `e4089a1`): `T?`/`.get_or`
      across `headers/spice/`.
- [ ] BSIM-class models — hand-ported to PHDL like everything else (user
      decision 2026-07-18: **all** models are native PHDL; OSDI is an interop
      path for external models, never the home of the stdlib). Big, phased:
      start from the ngspice C sources, one level at a time.

### Performance

Done: device bypass (per-variable-threshold stamp cache, suppressed while a
limiter clamps), matrix reuse (symbolic LU reused for the whole run),
transient predictor (CP-16 first-order Newton seed). Remaining:

- [x] **Temperature sweep — DONE 2026-07-18** (`5dfa04d`): `tnom` rescaling
      audited uniform across stdlib models; host-level `.temp` sweep proves
      the diode forward drop shifts ≈ −2 mV/°C (measured −1.66 mV/K at
      4.3 mA bias).

### Minor refactor leftovers

- [x] `digital/scheduler.rs` split — DONE (`2403e29`, 2026-07-16):
      topology/state/scheduler modules.
- [x] `DcAnalysisResult::as_iv` re-homed — DONE (`81b9c1d`):
      `Netlist::initial_values` owns the variable→reference mapping.
- [x] Shared `Integrator` — DONE (`81b9c1d`): noise trapezoid via
      `Integrator::trapezoid` in `math/integration`.
- [x] `SignalBridge` extraction — DONE (`1857df5`, 2026-07-13): owns the
      mixed-signal handoff in `core/circuit.rs`.
- [x] `Context::default` free of `init_global` — DONE (`81b9c1d`): solver
      builds own it, process-isolated test proves no leak.

### P1 named backlog (explicit non-goals for V1)

| Item | Disposition |
|------|-------------|
| `laplace_*`, `zi_*` operators | fail-loud; language backlog (user 2026-07-18) |
| LTRA (lossy tline, convolution) | urc covers the practical case; backlog |
| Autonomous-oscillator PSS | period detection needs phase conditions; backlog |
| AC `.sens` | DC ships; AC follow-up if the optimizer needs it |
| Exact-symbolic `∂R/∂p` sens | FD direct ships; upgrade behind the same API |
| `urc` | blocked on `codegen-parametric-devices` (T16) |
| Clocked digital fusing | comb cones fused; NBA-semantics follow-up |
| `.four`, `.pz`, `.disto`, `.sp` | niche, post-V1 |

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

Facade is docstringed and parity-tested (bench-removal). Governing rule:
**MD-22 — uniform host surface**: Python and Rust are one API; every item
below lands on both sides with the same shape.

- [ ] **`uniform-host-api` feature (MD-22):** Rust gains the object model
      (`load` → `Design` → `Module` → analyses, `compile()` →
      `LiveSession`, `InstanceView` indexing, bundle-shaped configs);
      Python gains the Rust-only knobs (nodeset, `dc_damp_tolerance`);
      naming unified (`Solver` vs `SolverConfig`, `const_`, `cross`
      direction enum). Working sheet: `docs/spec/appendix_c_host_surface.md`
      §4.

Remaining:

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

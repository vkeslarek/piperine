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

Pillars. V1 ships when the V1-marked ones are green.

| # | Pillar | One-line bar |
|---|--------|--------------|
| P1 | **Solver complete** | Every analysis a working SPICE user expects, plus PSS; engine gaps closed or explicitly documented as post-V1 |
| P2 | **Low-level device ABI** | ✅ Element ABI maturity complete (rollback, limiting, lifecycle, events, introspection) + PHDL introspection attributes delivered 2026-07-23 |
| P3 | ✅ **Python library polished** — CLOSED 2026-07-24 | `import piperine` is the single host: benches, validation, plugins scripting; documented, docstringed, stub-complete |
| P3b | ✅ **Blocking-bug fixes** — CLOSED 2026-07-24 | The gap-catalog items that *block a simulation or full user use* — `piperine build` stub, digital-codegen completeness, `.tf` correctness. |
| P4 | ✅ **Language server 100%** — CLOSED 2026-07-25 | Scope-aware resolution, project-wide navigation, attribute-schema + `///` doc-comment IDE support, protocol-level tests |
| P5 | **Plugin interface simplified** | One clear extension story (attributes + devices + hooks + scripts); native + Python backends only; writing a plugin is a documented afternoon task |
| P6 | **Cleanup & completeness** | Test sanitization (~800 tests → unit-inline, integration-grouped, dedupe), dead-flag/ignored-test removal, non-blocking codegen/interpreter completeness. Not all V1 — triage in the gap catalog. |
| P7 | **Optimizer** | Design-centering-capable optimization loop on the live-params engine; shape under study — PSS and `.sens` land first as its feeders |

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
  language-baked `@optimize` stays a To-Do design item under P7. PSS and
  `.sens` are needed regardless and land in P1.

---

## P1 — Solver complete

The merged open-gaps audit (ngspice-46 vs the native solver). **CLOSED
2026-07-18** (feature `p1-solver-complete`); `urc` shipped 2026-07-22 via
`hierarchy-flattening`; `laplace_*`/`zi_*` stay fail-loud. Every checkbox
below is done or in the named backlog table at the end of the section.

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
- [x] **`.four`/`.pz`/`.disto`/`.sp` — DONE 2026-07-19** (feature
      `spectral-analyses`, T1–T16): `.four` is `Waveform::fourier`
      post-processing (Rust direct-DFT, Python numpy, parity-tested), no
      solver analysis. `.pz` (poles via QZ on `(G,C)`, zeros via the
      Rosenbrock bordered pencil), `.sp` (per-port Thévenin excitation +
      power-wave S-matrix, ports declared via the `@rfport` attribute — no
      new device kind), and `.disto` (full Volterra HD2/HD3/IM2/IM3 from
      symbolic `disto2`/`disto3` JIT kernels) all ship on both hosts
      (MD-22). ngspice cross-checked for `.four`/`.pz`/`.disto`
      (`tests/ngspice_validation.rs`); `.sp` has no ngspice reference to
      cross-check against (documented Out of Scope).

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
- [ ] **`.disto` kernels overrun Cranelift on MOS2/MOS3 — OPEN, found
      2026-07-26** (p6-cleanup-architecture Phase 5, T29). The
      2nd/3rd-derivative kernels emit one JIT function per *ordered*
      controlling-branch combination, so a MOSFET's branch count makes the
      count explode and `cranelift-jit` panics with `TryFromIntError`
      (8 `ngspice_validation` MOSFET cases). Masked, not fixed: the kernels
      are now opt-in (`SessionBuilder::disto(true)`, default `false`) and
      `Session::disto` fails loud without them, so no host can solve against
      kernels that were never emitted. The real fix is to stop emitting the
      ordered cross-product — combinations are symmetric, and the derivative
      kernels should be one function with an index, not N functions. Until
      then `.disto` works on small-signal devices and is unavailable for
      MOS2/MOS3. The collapse found this because `Session::compile` used to
      emit these kernels *unconditionally*, so the surviving host entry point
      could never JIT a MOS2/MOS3 circuit at all; only the deleted
      `SimSession` path (which gated them off) could.
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
- [x] `urc` lumped RC line — DONE (`hierarchy-flattening` feature,
      `c15048d`…`a8ef83a`): authored as pure-structural fixed-N modules
      (`urc2`/`urc5`/`urc10` in `headers/spice/urc.phdl`) over a reusable
      `urc_seg` mid-level submodule. The `FlattenHierarchy` elaboration pass
      (non-destructive — writes only `Design::flat_modules`, never
      `Design::modules`) inlines the 3-level hierarchy
      (Top → urcN → urc_seg → res/cap) into the leaf-only flat netlist
      codegen consumes, so `circuit.rs:389`'s "nested hierarchy" guard is
      unreachable for valid input. ngspice cross-checked at lump 2/5/10
      (`tests/ngspice_validation.rs`); monomorph/restamp regression guards
      in `tests/urc_compile_count.rs`. The parametric `urc[N]` +
      `StructuralFor` route (cleaner per-shape monomorph) is deferred behind
      gap-3 (array-net expansion in flatten, fail-loud today) — the fixed-N
      route exercises the same 3-level inlining and ships now. LTRA (lossy
      tline, full convolution) — **backlog**: urc covers the practical
      lossy case.
- [x] Combined transformer block — DONE (T17, `678dcfe`): `xfmr(l1, l2, k)`
      over the mutual-flux engine; AC ratio and coupled-LC energy transfer
      validated.
- [x] Stdlib off sentinel params — DONE (T18, `e4089a1`): `T?`/`.get_or`
      across `headers/spice/`.
- [ ] BSIM-class models — hand-ported to PHDL like everything else (user
      decision 2026-07-18: **all** models are native PHDL; OSDI is an interop
      path for external models, never the home of the stdlib). Big, phased:
      start from the ngspice C sources, one level at a time. Two codegen
      needs surfaced while scoping BSIM, both now correctly framed:
      - **Internal nodes** (series-R, NQS, thermal) — **DONE at the codegen
        level**: a non-port `wire` already gets a fresh MNA unknown
        (`circuit.rs:419`); the BJT ships 3 (`cp/bp/ep`). No allocation work
        needed for PHDL devices.
      - **Self-heating** (`rth0`/`cth0`) — **authorable today**, not blocked:
        internal thermal `wire` + `ddt` cap + `Pdiss` current source +
        `V(thermal)` fed back into temp-dep params, all existing primitives;
        the electro-thermal coupling Jacobian is auto-derived by symbolic
        diff. Just needs a model + validation — tracked as `self-heating`.
      - **Multi-segment / NQS structure** (variable segment count) — needs
        `hierarchy-flattening` (see Devices above): the MVP flatten pass, plus
        two deferred sub-gaps (const-arg-into-behavior, array-net expansion)
        that bite only certain authoring routes. This is the real codegen
        work — structural monomorphization (`mod Foo[N]`) already exists.

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
- [ ] `bound_step_hint` has a codegen producer (`codegen/device/mod.rs`) but no
      solver consumer — wire it into the transient stepper or remove it
      (found by solver-simplification Verifier, 2026-07-19).
- [ ] Part VII §16 failure rows unenforced at runtime: empty-capabilities
      check, digital event → nonexistent net validation, digital boundary
      stability check. Either enforce or drop from the contract.

### P1 named backlog (explicit non-goals for V1)

| Item | Disposition |
|------|-------------|
| `laplace_*`, `zi_*` operators | fail-loud; language backlog (user 2026-07-18) |
| LTRA (lossy tline, convolution) | urc covers the practical case; backlog |
| Autonomous-oscillator PSS | period detection needs phase conditions; backlog |
| AC `.sens` | DC ships; AC follow-up if the optimizer needs it |
| Exact-symbolic `∂R/∂p` sens | FD direct ships; upgrade behind the same API |
| `urc` | DELIVERED via `hierarchy-flattening` — fixed-N pure-structural modules, ngspice cross-checked at lump 2/5/10 |
| Clocked digital fusing | comb cones fused; NBA-semantics follow-up |

---

## P2 — Low-level device ABI (`libloading` + PHDL declaration) ✅ CLOSED (2026-07-23)

**DELIVERED** — the reflective half by `element-abi-maturity`, the declarative
half by `phdl-introspection-attributes` (both with Verifier PASS; see the
follow-up block below). One Minor residue is listed there.

The plugin device path exists (native backend, `@device(plugin=…)`,
`DeviceProvider`). OSDI/ngspice are used as a **checklist for integration
maturity, not as the native ABI** — the native contract stays
mixed-signal-first; OSDI wrappers are one client.

**V1 blockers**

- [x] **Explicit lifecycle — DELIVERED 2026-07-23** (feature
      `element-abi-maturity`, 30 commits, 814 tests, Verifier PASS). Rollback
      checkpoint/restore on the `Element` trait (limiter + digital registers
      rewound on rejected steps — proven correctness bug fixed); formal
      `LimitingReport` API (replaced `limiting_active`/`convergence_hint`);
      lifecycle contract documented in Part VII §19 (hook chart + algorithm
      flow per analysis) + enforced by executable contract test.

**Not blockers (recorded to prevent re-litigation)**

- **Internal-unknown allocation (`HAS_INTERNAL_UNKNOWNS`):** the
  `allocate_unknowns` seam + capability flag are **implemented and tested**
  (`element.rs:149`, `builder.rs:144-160`; solver tests `parity_baseline.rs`,
  `composed_element.rs`, `live_params.rs`). Any `Element` returned by
  `DeviceProvider::build` (`plugin.rs:63`) allocates internal nodes through the
  standard builder path — identical to a native element. `@device(plugin =
  "osdi")` works once an OSDI `DeviceProvider` is wired; the only "fail loud
  today" is "no provider wired" (`plugin.rs:78`) — the plugin's job, not a
  solver gap. The stale `solver-osdi-abi-completion` spec listed this as
  pending; superseded by the shipped `solver-abi` feature.
- **PHDL internal nodes:** non-port `wire` → anonymous MNA unknown
  (`circuit.rs:419`); the BJT ships 3. These never touch
  `HAS_INTERNAL_UNKNOWNS` — that path is solver-native elements only. PHDL
  self-heating is a modeling task (`self-heating` feature), not a blocker.
- **Model/instance separation:** **rejected for the solver** (user decision
  2026-07-16, `solver-abi/spec.md:57`). It is a SPICE concept (shared `.model`
  card); Piperine is HDL-centric — the module is the model, each instance is a
  compilation/restamp. The sweep rebuild rule is owned by
  `ParamDescriptor::invalidation` (partially landed) + compile-once/restamp
  (MD-18). The OSDI wrapper (`piperine_osdi`, external) handles model/instance
  internally; the solver never sees the distinction.

**Element ABI maturity checklist — DELIVERED 2026-07-23** (feature
`element-abi-maturity`, `.specs/features/element-abi-maturity/`). Full spec
(48 requirements, 10 stories), design (12 tech decisions), tasks (32 atomic),
validation (PASS — 48/48 ACs, 5/5 sensor). The codegen bridge is complete
(`PiperineDevice` overrides every `Introspect` method with real kernel data).
The PHDL language declarations are a follow-up (bottom of this section).

- [x] **Commit/rollback for all mixed-signal state — DONE.**
      `ElementCheckpoint` + `checkpoint_state`/`restore_state` on `Element`;
      wired into transient reject path + DC homotopy retry; limiter + digital
      registers rewound. Accept-gated state naturally safe.
- [x] **Formal limiting API — DONE.** `LimitingReport { net, proposed,
      limited_value, limiter_name, reason }` replaces `limiting_active` +
      `convergence_hint` (both removed, zero dead refs). Codegen Limiter
      produces the report; Newton gate + DC bypass consume it.
- [x] **Lifecycle contract — DONE.** Part VII §19: per-analysis hook chart +
      algorithm flow (DC/AC/tran/noise/PSS/`.sens`); executable contract test
      in `lifecycle.rs` asserts ordering.
- [x] **Temperature protocol — DONE.** `set_temperature` wired into
      `CircuitBuilder::build`; per-instance `dtemp` composed into effective
      temp; `Invalidation::Temperature` driven by sweeps.
- [x] **Jacobian/stamp capability declaration — DONE.** `HAS_DISTO2`/
      `HAS_DISTO3`/`NUMERIC_JACOBIAN` bits; `.disto` warns when no device
      contributes, fails loud for numeric-only.
- [x] **Terminal/opvar catalogs — DONE (ABI bridge).** `TerminalKind` added;
      `PiperineDevice::list_terminals` bridges kernel terminals (positional
      External/Internal); `read_opvars`/`list_queries` bridge compiled var-eval
      path. `ModelDescriptor { type_id, version }` on Introspect.
- [x] **Save/probe selection — DONE.** `ObservableDescriptor` +
      `ProbeSelection`; `collect_device_banks` filters per-observable; fail-loud
      on unknown observable.
- [x] **Unified event model — DONE.** `EventQueue<EventEntry>` (BinaryHeap);
      four sources adapted (digital, breakpoints, scheduled sets, `$bound_step`);
      `predict_step` reads from unified queue; per-entry `RollbackBehavior`.
- [x] **`StepperStrategy` fold — DONE.** `ConvergencePlan` owns stepper
      alongside Newton + Homotopy; transient delegates; parity baselines
      bit-identical.
- [x] **Introspect leftovers — DONE (ABI).** `ModelDescriptor`; kernel named
      catalogs (state/force/noise terminals) surfaced through the ABI.
- [x] **Noise metadata — DONE.** `Noise { name, kind }` +
      `NoiseContribution { element, source, kind, psd, integrated_sq }`;
      per-source reporting end-to-end with conservation test
      (`noise.rs:437-502`).
- [x] **Parameter invalidation — DONE (core).** `ParamDescriptor::invalidation`
      wired through `CircuitInstance::set_element_param`, DC restamps, transient
      `apply_scheduled_sets`, `.sens` rebuild gate, Python host auto-rebuild.
- [x] **`NewtonStrategy` fold — DONE.** Already in `ConvergencePlan::newton`;
      `DampedNewton` wired.

**Follow-up: PHDL introspection attributes (language declarations) —
DELIVERED 2026-07-23** (feature `phdl-introspection-attributes`:
`spec.md` 4 stories / 20 requirements PIA-01..20, `design.md`, `tasks.md` 8/8,
`validation.md` independent Verifier **PASS** — 19/20 ACs matched, 3/3 sensor
mutations killed, gate 849 passed / 0 failed). The ABI/codegen bridge above is
reflective (host reads kernel data); this feature added the declarative half, so
a device author controls every catalog from PHDL.

**Design shape (user 2026-07-23): atomic single-purpose attributes, not role
bundles.** Metadata is expressed with small composable attributes — `@name`,
`@unit`, `@description`, `@kind`, `@model`, `@version` — each legal on multiple
declaration kinds, rather than one bundled `@opvar(...)`/`@observable(...)`/
`@terminal(...)` per role. Model identity is the one deliberate pair —
`@model(type, version)` carries both fields together. Every attribute is
optional (zero regression) and a
textual `extern attribute` declaration (MD-24, LSP go-to-definition). Payoff:
the opvar-name vs observable-name inconsistency *dissolves* — one `@name` on a
`var` feeds both catalogs, nothing to unify; `@kind` is placement-resolved
(`var` → `ObservableKind`, terminal → `TerminalKind`).

- [x] **Model identity** — `@model(type = "mos", version = "3")` on modules
      populates `ModelDescriptor` instead of echoing the module name (PIA-01..04).
      `device/mod.rs:569-586`; `codegen/tests/model_descriptor.rs`.
- [x] **Var metadata** — `@name`/`@unit`/`@description`/`@kind` on vars name and
      annotate the opvar query catalog AND the observable catalog from one
      declaration; replaces positional `ddt[k]` observable naming (PIA-05..09).
      One `var_display_name` helper feeds both (`device/mod.rs:161-171`);
      `codegen/tests/{opvar_bridge,observable_catalog}.rs`.
- [x] **Terminal classification** — `@name`/`@kind`/`@description` on ports and
      internal wires feed `TerminalDescriptor` (external/internal/auxiliary).
      A **new** attribute family, NOT `@port` (plugin device-wiring, plugin-only
      scope) and NOT `@rfport` (RF S-param ports) — distinct purpose, distinct
      surface (PIA-10..14). `device/mod.rs:503-559`;
      `codegen/tests/terminal_bridge.rs`.
- [x] **Named limiters** — the `$limit(kind, …)` `kind` now reaches
      `LimitingReport.limiter_name`/`reason` through a per-slot catalog on
      `AnalogKernel` (`kernel/analog/limits.rs:24-52`,
      `kernel/analog/mod.rs:433-439`), so a MOSFET distinguishes `fetlim` from
      `limvds` instead of reporting a hardcoded `"pnjlim"`/`VoltageStep`. An
      operator argument, not an attribute (PIA-15..18).
      **Residue:** PIA-16's *optional explicit reason* argument is a
      design-approved MVP deferral — the reason is inferred from the kind
      (`limvds`→`VdsStep`, else `VoltageStep`); no stdlib model needs an
      override today.
- [ ] **Residue — orphan-metadata check (Minor, from the PIA Verifier).**
      `@unit`/`@description` on a **shadowed or non-opvar** var is accepted by
      the lang resolver (shadowing is a codegen concept, unknowable at
      elaboration) and then dropped by codegen — the annotation silently does
      nothing. A codegen-boundary orphan check is the tracked remedy (lesson
      L-012); not a correctness bug.

---

## P3 — Python library polished ✅ CLOSED (2026-07-24)

**DELIVERED 2026-07-24** — feature `host-library`
(`.specs/features/host-library/`: `ideal.md` north-star, `delta.md` gap map,
`spec.md` 6 stories / 28 requirements HOST-01..28, `tasks.md` 30/30 tasks
done, `validation.md` Verifier PASS). The surface was designed greenfield
("the perfect `import piperine`"), diffed against what shipped, then closed
task by task. Governing rule **MD-22 — uniform host surface**: Python and
Rust are ONE API (same names, call shape, config/result types, errors),
enforced by a parity test (`tests/host_parity.rs`); Rust is designed *with*
Python, never bolted on.

Prior gap (verified 2026-07-23, now closed): the engine shipped far more than
the host exposed. The Rust host ran `sens`/`pss`/`pz`/`sp`/`disto` but Python
had no typed result class for them; `tf` existed only in the solver; the
entire `element-abi` introspection catalog (opvars, observables, terminals+
kind, model descriptor, limiting reports, per-source noise, param bounds) had
almost no host door.

All workstreams delivered (scope stayed host-pure — `optimize`→P7,
plugin-scripting→P5):

- [x] **#1/#2 (P1 MVP) — compiled `Session` center + uniform analyses.**
      `Session` is the host center on both hosts (Python `LiveSession`→`Session`;
      Rust equivalent built). Every analysis (`op`/`dc`/`tran`/`ac`/`noise`/
      `tf`/`sens`/`pss`/`pz`/`disto`/`sp`/`four`) ships on both hosts, kwargs-
      first, typed result. MD-22 breach closed; parity test locks it
      (`tests/host_parity.rs`).
- [x] **#4b (P2, highest leverage) — element-abi introspection door.**
      `inst.opvar`/`opvars`/`observables`/`model`/`terminals`(+kind);
      `trace.opvar` via `probe=`; `op.stats.limiting`; noise `by_source`/
      `contributions`; `Param.bounds`/`unit`/`scope`. Unlocks the opvar/
      efficiency driving scenario, convergence debugging, probe discovery, and
      auto knob-bounds for the P7 optimizer.
- [x] **#5 (P2) — return-type consolidation.** `Trace<T>` generic; folded
      `AcTrace`/`NoiseTrace` (nine-type taxonomy, `ideal.md` §6).
- [x] **#4 (P3) — rich `Waveform`.** Measurements (`slew_rate`/`rise_time`/
      `fall_time`/`overshoot`/`settling_time`/`delay`), transforms
      (`fft`/`resample`/`derivative`/`integral`/`clip`), `ComplexWaveform`
      margins/bandwidth (`bandwidth_3db`/`gain_margin`/`phase_margin`/
      `unity_gain_freq`), `plot`/`pip.plot`/`bode` (matplotlib-guarded).
- [x] **#3 (P3) — first-class sweeps.** Fluent `Session::sweep()`,
      nested/named `sweep_grid`, `SweepPoint`-as-`Session`, `Grid::map()`→
      ndarray/nested-Vec (compile-once, MD-18).
- [x] **#6/#7/#9 (P3) — configs, units, errors, discoverability, naming.**
      Typed kwargs/`__init__` with `.with_()`, canonical `Solver` knobs
      (nodeset, `dc_damp_tolerance`), SI unit newtypes (`Freq`/`Time`,
      `From<&str>`) + Python `pip.Hz`/`ns`/`mV`/`C`, typed `SimulationError`
      hierarchy, `NetRef` ergonomics, `CrossDirection`/`Scale` enums, complete
      hand-written `.pyi` stubs, property/method consistency, `const`/
      `design[name]`/`load_str`, `pip.extract`.
- [x] **Docs — `docs/spec/part_viii_host_api.md` + `appendix_c` updated** to
      the delivered surface (HOST-27/28).

**Deferred (recorded, out of this feature's scope):** `pip.optimize` (design
centering) stays **P7** — this feature made the optimizer *feedable*
(`Param.bounds`, opvar objectives) but did not implement the loop. Python
plugin scripting (`@pip.device`/`@pip.hook`, lifecycle-registry-to-Python)
stays **P5**. `HookInput.solve` payloads for swept analyses fold into future
introspection/hooks work. Packaging/PyPI stays post-V1 (module layout already
PyPI-shaped). A handful of `SPEC_DEVIATION`s were recorded along the way
(instance- vs module-scoped param access, dict-kwargs vs literal `a=`/`b=`
sweep spelling in Python, `Session::set` structural-change behavior) — see
`.specs/features/host-library/tasks.md` per-task Status notes and
`validation.md` for the full list; none contradict a spec goal, all are
phrasing-level.

Post-V1 interactivity (oscilloscope, dashboards, sliders driving `Session.set`)
builds on this — see gallery.

---

## P3b — Blocking-bug fixes (post host-sanitization) ✅ CLOSED (2026-07-24)

**DELIVERED 2026-07-24** — feature `p3b-blocking-fixes`
(`.specs/features/p3b-blocking-fixes/`: `spec.md` 8 requirements PB-01..08,
`tasks.md` 5/5 tasks done, `validation.md` Verifier PASS). The gap-catalog
rows that **block a simulation or full user use** — bugs, not features.
Every `file:line` claim was re-verified against current source before
fixing (code had moved since the gap catalog was written); one claim
(`.tf`'s "wrong number") turned out to be **stale** — see the correction
below.

- [x] **`piperine build` was a stub** — now elaborates the target file(s)
      and runs the full codegen pipeline (`lower_bodies` +
      `CircuitCompiler::build_circuit`) on every zero-port module (the
      only structural "circuit root" signal available — no `top` marker
      exists in `Piperine.toml` or PHDL); elaboration/codegen failures both
      exit non-zero, attributed to their module. Commit `2fa0e69`.
- [x] **Digital codegen completeness** (analog had these, digital rejected
      them — valid-looking digital designs now compile):
      - user-`fn` inlining in a digital body — tree substitution mirroring
        the analog inliner.
      - enum-pattern `match` in digital — a lowering-time pre-resolution
        pass rewrites the pattern into the discriminant literal, reusing
        the existing `Pattern::Literal` emission path.
      - real ↔ 4-state (`Quad`) conversion in digital — `Quad -> Real`
        reuses the existing route unchanged; `Real -> Quad` checks
        truthiness directly (SPEC_DEVIATION: the literally-specified
        `Real -> Int -> Quad` route would truncate a fractional nonzero
        value like `0.5` to `0` first, contradicting its own stated
        semantics).
      - (`for` in a digital body — still niche; stays parked in P6.)
      Commit `dae7e71`.
- [x] **`.tf` "wrong number" claim — corrected, not fixed as described.**
      Re-verification traced the call graph and found the `1e20`
      current-source placeholder was **provably unreachable**:
      `calculate_gain` runs first and already fails loud on any
      non-voltage input, so `calculate_input_resistance`'s dead `else`
      branch could never execute — no user could hit a live wrong number.
      Removed the dead branch and asserted the invariant
      (`debug_assert_eq!`) so a future refactor that reintroduces a
      current-source path here fails loud instead of silently reviving the
      wrong `1e20`. Commit `b600700`.

> Framing (user 2026-07-23): "super important" = blocks a simulation OR causes a
> bug that prevents full user use. Everything else from the catalog is P6.

---

## P4 — Language server 100% ✅ CLOSED (2026-07-25)

**DELIVERED 2026-07-25** — feature `language-server`
(`.specs/features/language-server/spec.md`; 26 requirements LSP-01..26,
`tasks.md` 23/23 tasks done, `validation.md` Verifier PASS). Audit finding
(2026-07-23): the server advertised 17 LSP capabilities but the **depth**
was thin — resolution was word-based global lookup, references/rename were
text scans, there was no project-wide symbol index, elaboration stopped at
the first error, and PHDL had no doc comments. The refinement deepened the
existing capabilities rather than adding new ones.

**"100%"** = the editor understands PHDL as well as the compiler does: names
resolve by scope (not first-match), navigation works across the whole project,
every diagnostic shows at once, attribute schemas and **doc comments** drive
hover/completion, and the whole thing is protocol-tested.

### Resolver correctness (the engine the IDE reads)

- [x] **Scope-aware name resolution** — `ResolutionIndex`/`BindingId` built as
      an elaboration side artifact; `resolve_at` maps cursor offset → use span
      → binding, replacing the old first-match global loop (deleted).
- [x] **Resolver-driven references/rename/highlight** — a shared occurrence
      engine (`occurrences(BindingId)`) backs references, rename, and
      document-highlight; comment/string text matches no longer leak in.
- [x] **Error-accumulating elaboration** — a new additive entry point
      (`elaborate_with_context_accumulating`) attempts every independent item
      in the two passes where independence genuinely holds (module
      elaboration, typecheck) instead of stopping at the first error; every
      other pass stays fail-fast (each is a real precondition for the next).

### Project-wide navigation

- [x] **Project-unit elaboration** (`ServerState.projects`/`ProjectUnit`):
      cross-file goto/rename, per-file diagnostic fan-out. SPEC_DEVIATION:
      one `Design` per file (keyed by path), not a single merged multi-file
      `Design` — no cross-file `Design`-merge primitive exists in
      `piperine-lang`; the "one binding index spanning files" requirement is
      delivered via a merged `ResolutionIndex`.

### IDE features (hover / completion / outline)

- [x] **Attribute-schema IDE support**: `@schema` completion, attribute-
      argument validation (unknown/mistyped/missing-required field →
      diagnostic at the arg), hover→schema fields, goto→`@attribute`
      declaration, outline entries for attribute instances.
- [x] **PHDL doc comments, Rust-style `///`.** Full pipeline landed: lexer
      captures `///` runs as trivia attached to the next token; POM gained an
      additive `doc: Option<String>` field (module/port/param/var/instance/
      behavior, `#[serde]`-carried, MD-25-compliant); elaboration attaches the
      captured doc; hover prepends it as Markdown above the type/kind line.

### Tests

- [x] **Protocol-level tests** over `Connection::memory()`
      (`tests/protocol.rs`): init → didOpen → hover/completion/goto/
      references/rename round-trips, plus dedicated shadowing, doc-comment-
      on-hover, and cross-file goto/rename fixtures.

---

## P5 — Plugin interface simplified

**REFINED 2026-07-25 (ideal-first)** — feature `plugin-interface-v2`
(`.specs/features/plugin-interface-v2/`: `ideal.md` north-star, `context.md`
12 locked decisions D1–D12, `spec.md` 26 requirements PLG-01..26, `design.md`
5-phase plan). The refinement replaces the flat bullets below with a
declaration-coupled, native+Python surface.

**Locked shape (D1–D12):** a **plugin is a contributing dependency** —
`piperine add <git>` (Go-style: bare `owner/repo`→GitHub, any full git URL)
adds a dependency; if its `Piperine.toml` declares scripts/devices/hooks,
importing it loads them (a normal project can declare the same). Three
shapes under one umbrella: **pure-PHDL** (code lib), **scripted** (Python
scripts/hooks, no binary), **device** (a language-agnostic C-ABI binary).
Declaration + injection are coupled (no imperative `Registrar`): a device's
`@device pub mod` lives in the **plugin's own PHDL** and importing injects it
(the user never writes `@device`); scripts/hooks are declared+bound by a
single decorator with **literal Rust/Python parity** (`#[pip::script]`/
`@pip.script`, etc.). **No plugin `extern`/attribute schemas** (the
`extern.phdl` stub mechanism is deleted). Device binaries distribute via
**GitHub release + target-triple + TOFU** (`device = { release =
"github:owner/repo@tag", verify = "sha256" }`); a missing triple is a loud
error, `verify` is optional. Two explicit trust gates at `add`: **permissions
consent** + source/binary TOFU. The **five lifecycle hooks are frozen**;
`transform_design` (staging) is the sole device-injection point.

**DELIVERED 2026-07-25** — all 17 tasks (T1–T17) executed and validated;
`.specs/features/plugin-interface-v2/validation.md` is the evidence.

- [x] Remove the WASM (wasmtime) and process JSON-RPC backends +
      `piperine-plugin-wasm`; native dlopen stays (MD-21).
- [x] Kill the imperative `Registrar` + per-plugin `extern.phdl` stubs;
      contributions are declaration-coupled decorators / `@device`.
- [x] Decorator surface with literal Rust/Python parity + a parity test
      (`tests/plugin_parity.rs`).
- [x] Device-binary distribution: git-source resolver, github-release +
      triple match, TOFU pin, permissions-consent gate at `add`.
- [x] One "write a plugin" document per shape (pure-PHDL, scripted, device)
      showing the Rust/Python decorator equivalence
      (`docs/spec/part_vi_plugins.md` Appendix A).

**Deferred to a follow-up (D12):** a manifest `[plugin] piperine = ">=X.Y"`
version-compat field for the source/script surface — v2 relies only on the
device binary's exported `piperine_plugin_abi_version`; the manifest field's
semantics are easy to bolt on later. Also deferred: a committed-in-repo
device-binary offline fallback (v1 is release-only), and any hooks beyond
the five.

---

## P6 — Cleanup & completeness

**HYGIENE SUBSET DELIVERED 2026-07-26** — feature `p6-cleanup-completeness`
(`.specs/features/p6-cleanup-completeness/`: spec CLN-01..21, `audit.md` is the
measurement, `validation.md` the evidence). The numbers this section used to
quote were stale; corrected below.

- [x] **Test sanitization.** The real inventory was **1123 passing tests / 179
      targets**, not ~800. Applied crate by crate against **MD-28**, suite green
      after every commit: the plugin manifest suite moved inline (14),
      `lang-server`'s 1880-line `integration_test.rs` split into nine
      feature suites (43 tests, shared harness in `tests/common/`),
      `plugin/tests/phase3.rs` split into staging/hooks/scripts, four targets
      renamed off dead vocabulary (`codegen_ir`→`analog_device_numerics`,
      `from_ir`→`circuit_from_design`, `cli_check`→`check_cmd`,
      `protocol_surface` folded into `protocol`), and the example-elaboration
      gate — which existed in **three** same-layer copies — reduced to the root
      one. Every deletion names its surviving equivalent.
- [x] **Dead / ignored test cleanup.** The "28 ignored" claim was wrong in a
      worse way than stale: those attributes sat inside
      `piperine-codegen/tests/ppr_ir.rs`, whose first line is `#![cfg(any())]`,
      so the file **never compiled** — and `analog_jit.rs` (412 lines, 11
      tests, listed in `CLAUDE.md` as a test of record) was dark the same way.
      38 dead tests → 20 restored against today's API (`resolve_lowering.rs`,
      `analog_kernel.rs`), 18 deleted with survivors named. The tree now holds
      **zero** `#[ignore]` attributes and zero switched-off test code; the only
      ignores left are 4 illustrative doctest fences, each registered with its
      reason. `tests/suite_hygiene.rs` guards all of it (and requires every
      integration target to declare its scope in a `//!` header).
- [x] **Dead capability flags.** `SUPPORTS_QUERIES` had zero declarers and zero
      readers → **removed** (`1 << 10` left vacant). `BYPASS_OK` was declared by
      one *test* device and never consulted, while the DC bypass cached stamps
      for every circuit → **wired**: the cache is now gated on every element
      declaring it, and codegen declares it only when a device's DC stamps are a
      pure function of its terminal voltages. `capabilities_contract.rs` now
      fails on any entry that says "reserved" or "no consumer".
      **`bound_step_hint` was never dead** — producer in `codegen/device/mod.rs`,
      consumer at `analyses/events.rs:326`, test in `event_adapters.rs`.
- [x] **Part VII §16 failure rows.** The table has **16** rows and now carries an
      *Enforcement* column: 8 name the test that trips them (§9, §12 ×2, §15 and
      §18's validation half are new in `failure_rules.rs`), 8 are marked
      *not yet enforced* — reachable but unchecked, listed below as residue.
      `spec_failure_rules_guard.rs` fails if a row is neither bound nor marked,
      or names a test that no longer exists.
- [ ] **§16 residue — the eight unenforced rules.** §2 (a no-capability element
      is silently inert; two fixtures rely on that), §4 ×2 (no per-analysis
      boundary snapshot; an out-of-range digital net is not constructible), §6
      (an unmapped `AnalogReference` is not constructible), §8, §10 (`dt_min`
      floor untested), §11 (singular AC point untested), §14 (non-settling delta
      cycle untested). Each is a behaviour addition or a fixture that P6's
      hygiene scope deliberately did not take on.
- [ ] **User manual** — `docs/manual/` held a single placeholder `index.md`
      promising guides nobody wrote, and was removed with its `mkdocs.yml` nav
      entry (P6/T13, 2026-07-26). Writing the manual (CLI walkthrough, library
      usage, toolchain guide) is a real deliverable and lives here until it is
      scheduled; recreate the directory *with content* when it is. Today's usage
      documentation is `README.md`, `CLAUDE.md`, and spec Appendix A.
- [ ] **Non-blocking language/interpreter completeness** — slice expressions
      outside analog/digital bodies (`eval/interp.rs:448`); `for` in a digital
      body (`emit/stmt.rs:98`); selector complex-exprs / field-less match
      (`pom/selector/*` — overlaps the language-backlog selector-axes item,
      decide as one). None blocks common use; schedule on demand. **Post-V1.**

> Delivered: test sanitization + dead/ignored-test cleanup + dead-flag triage +
> §16 enforcement. Remaining: the §16 residue and the completeness items, both
> post-V1 unless a user hits them.

---

## P7 — Optimizer

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

## Gap catalog — V1 triage (candidates, to discuss)

> **Added 2026-07-23** from a full codebase sweep (TODO/FIXME, `todo!`,
> fail-loud `Unsupported`, provisional "for now/not yet" markers, `#[ignore]`d
> tests, dead flags). The tree is clean — **1 `TODO`, zero `todo!()`/
> `unimplemented!()`** — so these are the real remaining gaps, each with
> `file:line` evidence and a **proposed** disposition. Nothing here is decided;
> this is the sheet to sort **V1 / post-V1 / drop** against.

### A. Tooling / CLI

| Gap | Evidence | Proposed |
|-----|----------|----------|
| **`piperine build` is a stub** — sets up headers, prints "Building…", but never calls the compiler/elaborator | `piperine-cli/commands/build.rs:33` (`// TODO: call compiler/elaborator`) | **V1** — a build command that doesn't build is a hole |
| **28 `#[ignore]`d tests**, incl. the whole `ppr_ir.rs` "pending POM Stmt rewrite" | `piperine-codegen/tests/ppr_ir.rs` (10×), + 18 others | **V1 decide** — fix or delete the stale `.ppr`/IR test path; ignored tests rot |

### B. Digital codegen completeness (analog has it, digital doesn't)

| Gap | Evidence | Proposed |
|-----|----------|----------|
| User-`fn` inlining in a **digital** body (analog path works) | `codegen/emit/builder.rs:334` | V1? — parity with analog `fn` inlining |
| Enum-pattern resolution in digital | `codegen/emit/stmt.rs:234` (`enum resolution not yet wired`) | V1? — `match` on enums in digital |
| `for` loops in digital emit | `codegen/emit/stmt.rs:98` | post-V1? — structural `for` exists; body-`for` niche |
| real ↔ 4-state conversion in digital | `codegen/emit/builder.rs:679` | decide — how often needed? |

### C. Language / interpreter / selector

| Gap | Evidence | Proposed |
|-----|----------|----------|
| Slice expressions outside analog/digital bodies (interpreter) | `piperine-lang/eval/interp.rs:448` | post-V1? |
| Selector: complex exprs skipped, field-less match returns false, `AxisNotImplemented` | `pom/selector/{parse.rs:141,eval.rs:122}`, `pom/error.rs:235` | overlaps the language-backlog selector-axes item; **decide as one** |
| `laplace_*` / `zi_*` operators (fail-loud) | already P1 backlog | post-V1 (locked) |
| Array-net expansion (`gap-3`) — blocks parametric `urc[N]` + some BSIM authoring | `elab/lower/flatten.rs:193` | tied to BSIM (P1); post-V1 unless BSIM needs it |

### D. Analyses edge cases

| Gap | Evidence | Proposed |
|-----|----------|----------|
| `.tf` input impedance for a **current-source** input returns a placeholder (`1e20`) | `piperine-solver/analyses/tf.rs:394` | V1? — small, correctness-shaped |
| Clocked-member network fusion (comb-only cones today) | `codegen/kernel/digital/compile.rs:487` | already P1 backlog (NBA semantics) |

### E. ABI / solver leftovers (some already logged under P1/P2)

| Gap | Evidence | Proposed |
|-----|----------|----------|
| ~~`SUPPORTS_QUERIES` capability flag~~ | — | **done (P6)**: removed; zero declarers, zero readers |
| ~~`bound_step_hint` producer, no consumer~~ | `analyses/events.rs:326` | **not a gap (P6)**: it has a consumer and a test — the note was wrong |
| Part VII §16 failure rows unenforced at runtime | `docs/spec/part_vii_solver.md` §16 | **partly done (P6)**: 8/16 bound to tests, 8 marked *not yet enforced* + guarded |
| Plugin **scripts** not runnable on out-of-host tiers | `pom/wire.rs:44` | folds into MD-21 (WASM/process tiers removed anyway) |

**Assigned homes (user 2026-07-23):**
- **→ P3b** (blocks simulation/full use): A `piperine build` stub · B digital
  codegen completeness (fn-inline, enum-pattern, real↔4-state) · D `.tf`
  current-source Zin.
- **→ P6** (cleanup & completeness): **test sanitization** (new) · A ignored-test
  cleanup · E `SUPPORTS_QUERIES`/`bound_step_hint`/§16 dead-flag triage ·
  C slice-outside-bodies + digital `for` + selector complex-exprs.
- **Already tracked** (leave in place): laplace/zi, LTRA, clocked fusing,
  array-net `gap-3`, BSIM (P1 backlog); selector axes (language backlog);
  plugin out-of-host scripts (folds into MD-21/P5).

Rows are still *candidates* — the pillar home is where each will be
checkbox-triaged for V1 vs post-V1, not a commitment to build.

---

## Post-V1 — plugin gallery (priority order, user 2026-07-23)

Priority: **MCU co-simulation → Python interactivity → schematic → PCB → the
rest** (OpenEMS EM cosim lives at the back — distinct from MCU cosim).

1. **MCU co-simulation** (top priority) — inject event-driven digital devices
   simulating AVR/ESP32-class MCUs (engines: Renode and/or Wokwi cores —
   possibly both, per target family); rides the P2 device ABI + commit/rollback.
2. **Python interactivity** — digital oscilloscope, dashboards with
   buttons/sliders bound to `Session` params, general ergonomics. **Built on a
   live-sim engine** (needs a solver/host capability, see below):
   - **Live-sim mode** — `tran` runs *continuously*, streaming per-step results
     to the host so a plot / "virtual oscilloscope" updates fluidly *while the
     simulation advances* — not batch-then-render.
   - **Stepping mode** — `tran` *waits for a host command* to advance (single
     step / run-to-time), for interactive inspection.
   - **Knobs-while-running** — the user drags a slider (`Session.set`) and the
     running live-sim reflects it on the next streamed step — play with the
     circuit as the scope updates. This is the interactivity payoff.
   - *Foundation needed:* a streaming/stepping transient driver that yields
     control to the host per accepted step (pausable/resumable), extending
     `LiveSession`/`Session` (`set`/`schedule_set` already exist). Post-V1 but
     the enabling engine work is the gating item.
3. **Schematic generation** — `@schematic(...)` attributes → rendered
   schematics from the POM (adoption driver).
4. **PCB export** — `@socket(socket = "DIP", ...)`-style attributes feeding a
   PCB generator/exporter.
5. **The rest:**
   - **Yosys bridge** — translate digital PHDL to Yosys for synthesis +
     open-source programmer flows.
   - **OpenROAD / OpenFASoC integration** — design params declared in-language
     via attributes; manage the flow from the HDL.
   - **Richer SPICE interop** — `@spice(symbol = "N", ...)` custom attributes.
   - **OpenEMS EM co-simulation** (deep backlog — distinct from MCU cosim) —
     couple the field solver for antenna/PCB/interconnect EM: OpenEMS computes
     the electromagnetic behavior, Piperine drives the terminal excitation and
     folds the extracted S-params/impedance back into the circuit solve (the
     `.sp`/`@rfport` surface is the seam). An EM block is one more `Element`
     client of the P2 device ABI.

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

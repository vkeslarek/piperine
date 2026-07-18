# ROADMAP.md — Piperine V1 and beyond

Rewritten 2026-07-18. Everything delivered before this date was purged (git
history + `.specs/` keep the record). Convention unchanged: **fail loud** —
what the toolchain cannot do is a named error, never a silent no-op.

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
| P5 | **Plugin interface simplified** | One clear extension story (attributes + devices + hooks + scripts); backend count reduced; writing a plugin is a documented afternoon task |
| P6 | **Optimizer** | Design-centering-capable optimization loop on the live-params engine; shape (library vs language) locked by study |

### Open architecture decisions (lock before the pillar work starts)

- **AD-pending: `piperine-api` crate topology.** The host plumbing
  (session/results/waveform/hooks) moved to the root crate in MD-19 and the
  root-as-lib layout is unsatisfying. Proposal on the table: a dedicated
  `crates/piperine-api` (pure Rust: host API + ABI contracts + plugin/device
  traits), `piperine-python` as a thin binding layer over it, root crate
  reduced to a re-export shell or removed as a package. Decide, record as
  MD-20, then move — one refactor, not incremental drift.
- **AD-pending: plugin backend reduction.** Three backends (native dlopen,
  WASM/wasmtime, process JSON-RPC) is a monster. Proposal: keep **native**
  (trusted, fast — also the P2 device path) and **Python** (plugins written in
  Python through the embedded host — same surface as benches); demote or drop
  WASM and process tiers. Decide before P5.
- **AD-pending: optimizer shape.** Library-first (scipy/nevergrad driving
  `LiveSession` — works today, MD-18 compile-once restamp loop is exactly what
  an optimizer needs) vs language-baked (`@optimize` attributes + native
  algorithms). Study in progress (design centering is the target use case);
  `.sens` (P1) feeds it sensitivities either way.

---

## P1 — Solver complete

Deep audit lives in `SOLVER_GAPS.md` (open items only). Summary of what V1
requires:

**Analyses**
- [ ] `.dc` sweep as a native solver analysis (today: host-level compile-once
      sweep — decide if that already satisfies V1 or if source sweeps need the
      solver loop).
- [ ] `.sens` (DC/AC sensitivity) — reuse the symbolic-diff machinery; direct
      feeder for the P6 optimizer.
- [ ] **PSS (periodic steady state)** — shooting method over the transient
      engine; required for switching converters and RF-adjacent work.
- [ ] `.four` — Python-side post-processing helper (numpy FFT on `Waveform`),
      not a solver analysis.
- [ ] `.pz`, `.disto`, `.sp` — niche, post-V1 unless a model needs them.

**Transient engine (TR-BDF2 core is done and active)**
- [ ] **Breakpoints** — the efficiency gate for switched circuits (edge
      thrashing ~40k steps today). Source-declared breakpoint schedule.
- [ ] Output interpolation onto the `.step` print grid.
- [ ] Enforced UIC hold (`.ic` clamp at t=0, released after).
- [ ] Remove vestigial `IntegrationMethod` enum (TR-BDF2 is the sole scheme;
      ~34 references left in the solver).

**Codegen/engine operator gaps (all fail loud today)**
- [ ] `table(x, xs, ys, mode)` — not even registered; register fail-loud, then
      implement 1-D interpolation companion.
- [ ] `transition`, `laplace_*`, `zi_*` — companion models.
- [ ] `idt` AC stamp (`1/jω`) — contributes 0 in AC today.
- [ ] Multiple `ac_stim` per contribution.
- [ ] `@initial` cannot force a branch (event bodies reject Force statements).
- [ ] `Trace.i` over time on devices with runtime state/vars (per-step
      state/var banks not recorded).

**Digital**
- [ ] Wire the fused combinational-network JIT (`NetworkComb`, built and
      tested standalone) into `circuit.rs::run_digital_at`; then fuse clocked
      members.

**SPICE model completeness ("everything I can do in spice, I can do here")**
- [ ] Fix MOS1 drain current ~1.5× (`headers/spice/mos.phdl` vs `mos1load.c`).
- [ ] Fix JFET ~15 mV bias discrepancy.
- [ ] MOS levels 2/3 (level 1 exists).
- [ ] Transmission lines (`tline`, lossy, `urc`).
- [ ] Combined transformer block (`ind`+`mut` as one device — the mutual-flux
      engine supports it; separate devices would double-force one branch).
- [ ] Migrate models off sentinel `$param_given` encodings onto `T?` optionals.
- [ ] BSIM-class models arrive via OSDI (P2), not hand-ported PHDL.

---

## P2 — Low-level device ABI (`libloading` + PHDL declaration)

The plugin device path exists (native backend, `@device(plugin=…)`,
`DeviceProvider`). What's missing for V1:

- [ ] **Internal-unknown allocation seam** — external models (OSDI first)
      need to allocate internal MNA nodes/branches before matrix
      finalization. This is the blocker for `@device(plugin = "osdi", …)`
      binding from PHDL (the `piperine_osdi` Rust API works; the PHDL seam
      fails loud).
- [ ] Element lifecycle formalized for external devices (setup → temperature
      → load → accept/rollback → destroy) — see SOLVER_GAPS ABI checklist.
- [ ] Artifact distribution — prebuilt plugin binaries per target triple from
      git releases (today the artifact must pre-exist).
- [ ] The MCU-simulation plugin (post-V1, see gallery) is the second client
      of this ABI — keep the contract wide enough for event-driven digital
      peripherals, not just analog compact models.

---

## P3 — Python library polished

Facade is docstringed and parity-tested (bench-removal). Remaining:

- [ ] `piperine.plot(waveform, ...)` convenience (matplotlib wrapper).
- [ ] `.four`-style post-processing helpers (`waveform.fft()`, etc.).
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

Part VI is implemented (manifest, TOFU, native/WASM/process backends, attr
schemas, `@device`, hooks, scripts). V1 is about **reduction and polish**, not
features:

- [ ] Execute the backend-reduction decision (AD-pending above).
- [ ] One "write a plugin" document with a worked example per extension kind
      (attribute schema, device, hook, script).
- [ ] Plugin-authoring in Python (if the backend decision lands there):
      register schemas/hooks/scripts from a `.py` plugin through the embedded
      host.
- [ ] Wire-tier scripts (capability-gated fs) — only if the process/WASM tier
      survives the reduction; otherwise delete the loud error with the tier.

---

## P6 — Optimizer

Target use case: **design centering** (maximize yield over process/tolerance
spread). Foundations already in place: compile-once restamp sweeps (MD-18),
`LiveSession` (`set`/`schedule_set`/rebuilds), `.sens` planned in P1.

- [ ] Study closure: pick the algorithm family (worst-case distance vs
      Monte-Carlo yield estimation vs ellipsoidal) and the shape (Python
      library vs language-baked) — AD-pending above.
- [ ] V1 deliverable: an optimization loop a user can run on a real circuit
      (params in, spec functions out, centered design back) with docs and an
      example.
- [ ] Language hooks (`@optimize`, tolerance annotations on params) only if
      the study says they pay for themselves in V1; otherwise post-V1.

---

## Post-V1 — plugin gallery (priority order sketch)

1. **MCU co-simulation** — inject event-driven digital devices simulating
   AVR/ESP32-class MCUs (candidate engines: Renode, Wokwi cores — evaluate);
   rides the P2 device ABI.
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

Condensed from the old roadmap; full design sketches in git history
(`ROADMAP.md` pre-2026-07-18).

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

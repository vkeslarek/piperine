# ROADMAP.md — Piperine V1 and beyond

**Rewritten 2026-07-28.** Prior delivery history lives in git and `.specs/`;
this document is forward-looking only.

Convention: **fail loud** — what the toolchain cannot do is a named error,
never a silent no-op. Cross-validation: `tests/ngspice_validation.rs`.

---

## North star

Piperine is a complete HDL-centric design toolchain for the **low/medium-level
designer**: hobbyists, independent professionals, and small teams building real
products without Cadence-class tooling. The gap we fill is *integration*: one
language and one host covering design entry, SPICE-class simulation,
mixed-signal, live/interactive simulation, design verification, optimization
(design centering), and — through plugins — schematic generation, PCB export,
and digital flows.

The **bench is Python**: PHDL describes circuits; everything procedural —
verification, sweeps, optimization, dashboards — lives in the Python host.

---

## V1 status

| Pillar | Status |
|--------|--------|
| P1 — Solver complete | ✅ closed (2026-07-22); backlog items below |
| P2 — Device ABI | ✅ closed (2026-07-23) |
| P3 — Python library | ✅ closed (2026-07-24) |
| P3b — Blocking fixes | ✅ closed (2026-07-24) |
| P4 — Language server | ✅ closed (2026-07-25) |
| P5 — Plugin interface | ✅ closed (2026-07-25) |
| P6 — Cleanup | hygiene subset delivered (2026-07-26); residue below |
| P7 — Optimizer | **design phase** — see below |
| DV — Design verification | **dv-core delivered** (2026-07-28); dv-gradients + dv-tester next |

**Delivered in V1 so far (~1360 tests, ngspice cross-validated):** every
analysis a SPICE user expects (DC/AC/tran/noise/PSS/sens/tf/pz/disto/sp/four),
TR-BDF2 transient with gmin/source stepping, Element ABI with rollback +
limiting + lifecycle, `import piperine` host with typed configs/results/sweeps,
LSP with scope-aware resolution + doc comments, native+Python plugin surface,
and the full design-verification core (`tol`, `constraint`, margins, findings,
postures, SOA).

---

## Forward-looking items

### P1 backlog (not V1-blocking)

| Item | Status |
|------|--------|
| `.disto` on MOS2/MOS3 — Cranelift function-count explosion | **open** — kernels are opt-in (`disto(true)`, default off); fix = stop emitting the ordered cross-product (combinations are symmetric → one indexed function, not N) |
| `laplace_*` / `zi_*` operators | fail-loud; language backlog |
| LTRA (lossy tline, convolution) | urc covers the practical case; backlog |
| Autonomous-oscillator PSS | period detection needs phase conditions; backlog |
| AC `.sens` | DC ships; AC behind the same API if the optimizer needs it |
| Clocked digital fusing | comb cones fused; NBA-semantics follow-up |
| BSIM-class models | hand-ported to PHDL; big, phased. Needs array-net expansion (`gap-3`) for certain authoring routes |

### P6 residue

- [ ] **§16 unenforced rules** (7 rows) — §2 (no-capability element silent),
      §4 ×2 (digital boundary / nonexistent net), §6 (unmapped variable —
      unreachable), §8 (dimension change), §10 (`dt_min` floor untested),
      §11 (singular AC point untested), §14 (non-settling delta cycle untested).
      Each is a behaviour addition or a driving test the hygiene scope skipped.
- [ ] **User manual** — `docs/manual/` was removed (placeholder only).
      Recreate with content when scheduled.
- [ ] **Non-blocking completeness** — slice expressions outside bodies,
      `for` in digital, selector complex-exprs. Post-V1.

### Design verification — the DV triad

**dv-core ✅ delivered** (2026-07-28, feature `dv-core`, 32 tasks, Verifier
PASS). What landed:

- `tol gauss(…)` on `param` — declared, inert, POM-visible
- `constraint Mod { require/var/target }` — grammar, elaboration,
  pointwise/reduced classification, monomorphization
- Signed margins (`m ≥ 0` ⟺ satisfied), `tol`-normalized, worst + argmin
  (time/freq/sweep + instance path)
- `validation_reports()` + `EMITS_VALIDATION` — cross-cutting findings channel
- Three postures: `strict` (default, aborts), `collect` (records), `off`
  (zero cost)
- SOA constraints on spice models (mos1/mos2/mos3/diode/bjt) — limits
  absent-by-default, vacuously satisfied
- `enum Region` opvar on MOSFETs
- Analysis-scoped event blocks (`@ dc`/`@ tran`/`@ ac`) + temporal windows
  (`after`/`dur`) + window algebra (`&`/`not`)
- `MarginsResult` channel + `r.violations` on both hosts, parity-guarded

**Next in the triad:**

- **dv-gradients** — analytic sensitivity (`∂F/∂p`) kernel, param-stamp
  Jacobian, host policies for Monte Carlo / corner runs / centering. The
  margin channel from dv-core is its objective function. AC/transient
  adjoint of a margin is a known gap (DC only in V1).
- **dv-tester** — programmatic tester, monitors, `cover` directive, batch
  verification at scale. Rides the findings channel for digital verification
  (margins are analog-only).

---

## P7 — Optimizer (design centering)

**Target:** maximize yield over process/tolerance spread. A user declares
`tol` distributions on params, writes `constraint` blocks for specs, and the
optimizer centers the design so every margin stays positive across the
declared variation.

**Foundations in place (dv-core + host-library):**

| Foundation | Status |
|---|---|
| Compile-once restamp sweeps (MD-18) | ✅ `Session::sweep`/`sweep_grid` |
| Live `Session::set` (restamp, no re-JIT) | ✅ Rust host |
| `.sens` (DC sensitivity, finite-difference) | ✅ both hosts |
| `tol` distributions declared on params | ✅ dv-core |
| Signed margins with argmin (the objective) | ✅ dv-core `MarginsResult` |
| `collect` posture (records worst margin) | ✅ dv-core |
| `Param.bounds` (auto knob-limits) | ✅ host-library |

**What's left to build (shape under design):**

- [ ] **Algorithm family** — worst-case distance vs Monte-Carlo yield vs
      ellipsoidal. No decision forced yet.
- [ ] **`∂F/∂p` analytic kernel** (dv-gradients) — the gradient feed the
      optimizer reads. Today only finite-difference `.sens` exists; the
      analytic path is faster and feeds centering directly.
- [ ] **Monte Carlo / corner sampling** — sample `tol` distributions, sweep
      each sample through `Session::sweep`, collect margins. The host
      reduction is straightforward once dv-gradients delivers the param-stamp
      infrastructure.
- [ ] **`pip.optimize` API** — one Python entry point: params in, spec
      functions out, centered design back. Shape TBD (library vs decorator).
- [ ] **Example + docs** — a real circuit centered end-to-end.

**Non-goals for V1:** high-sigma estimation, aging, worst-case distance
with non-Gaussian distributions. These are post-V1 once the basic loop ships.

---

## Post-V1 — plugin gallery (priority order)

1. **MCU co-simulation** (top) — AVR/ESP32-class digital devices via Renode/
   Wokwi cores; rides the device ABI + commit/rollback.
2. **Python interactivity** — live-sim mode (streaming transient), stepping
   mode, knobs-while-running (slider → `Session.set` → next step reflects it).
   Needs a pausable/resumable transient driver.
3. **Schematic generation** — `@schematic(...)` → rendered schematics from POM.
4. **PCB export** — `@socket(socket = "DIP", ...)` → PCB generator.
5. **The rest:** Yosys bridge, OpenROAD/OpenFASoC, richer SPICE interop,
   OpenEMS EM co-simulation.

---

## Language backlog (schedule on demand — none blocks V1)

- Capabilities for implicit rules (`From<T>` widening, intrinsic `impl Add`,
  `Iterable<T>`, `FromLiteral`)
- `extern` declarations (elaborator registration, prelude migration)
- Type system (tuple types, `fn`-reference typecheck, `var` inference,
  tuple destructuring, bundle-literal field defaults)
- Host addressability (net/instance arrays, `wire x : T[N]`)
- Selector axes (`driver::`/`load::`/`parent::`/`ancestor::`)

---

## Out of agent scope

VS Code extension productization, marketplace packaging, release/versioning —
`editors/vscode/`.

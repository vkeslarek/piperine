# ROADMAP.md — Piperine V1 and beyond

**Rewritten 2026-07-28.** Prior delivery history lives in git and `.specs/`;
this document is forward-looking only.

Convention: **fail loud** — what the toolchain cannot do is a named error,
never a silent no-op. Cross-validation: the per-domain targets
`tests/ngspice_{validation,ac,noise,tran,temp,homotopy}.rs` over the
`tests/ngspice/` fixture twins, with `tests/ngspice_parity_matrix.rs` as the
gate that keeps a ✅ claim falsifiable (a `Covered` cell names a fixture one
of the targets actually runs; everything else is a recorded `NotApplicable`
with a reason or a `pending: …` open).

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
| BSIM-class models | split into `.specs/features/ngspice-bsim3/` and `ngspice-bsim4/`, scaffolded `NOT YET SPECIFIED`. `gap-3` (array-net expansion in flatten) **landed**; the codegen integration that consumes it (lifting array-wire elements in the symbol table, processing structural `connections` in the circuit builder) is the open that currently blocks the generic-authoring route (`urc[N]` is the first proving case — see NGP-18 deviation in `ngspice-parity`) |

### SPICE model completeness — existing models

> Re-rewritten from **measured** parity results (the
> `tests/ngspice_parity_matrix.rs` registry is the source of truth). A ✅ below
> is a `Covered` cell — a golden fixture that fails when the claim breaks.
> Anything else is a named open, not an assertion. The 2026-07-28 audit table
> (which marked whole rows ✅ from a source-reading exercise, with no oracle to
> falsify them) is superseded.

**Existing models — measured faithfulness status:**

| Model | File | DC op (golden) | DC sweep | AC | Noise | Transient | Temperature | Open |
|---|---|---|---|---|---|---|---|---|
| res | `passives.phdl` | ✅ `divider` | ✅ `diode_iv` | ✅ `ac_rc` | ✅ `noise_rc` | ✅ `tran_rc` | pending: needs a tempco-bearing fixture | per-instance `bv_max` override missing |
| cap | `passives.phdl` | n/a (open at DC) | n/a | ✅ `ac_rc` | ✅ `noise_rc` | ✅ `tran_rc` | pending: no temp sweep | `ic` UIC declared but unwired (T16) — the `@initial` mechanism exists, models not yet wired to it |
| ind | `passives.phdl` | pending | pending | pending | n/a (reactive, noiseless) | pending | pending | `ic` UIC declared but unwired (T16) |
| sw | `switches.phdl` | pending | pending | pending | ✅ `noise_switch` | pending | pending | ON-state thermal noise landed (T13); negative hysteresis approximately right |
| csw | `switches.phdl` | pending | pending | pending | ✅ `noise_csw` | pending | pending | ON-state thermal noise landed (T13) |
| dio | `diode.phdl` | ✅ `rdiode` | ✅ `diode_iv` | pending | pending | pending | ✅ `temp_diode` | LEVEL=3 geometry path (w/l→area) missing; `ic` UIC declared but unwired (T16) |
| jfet | `jfet.phdl` | ✅ `jfet_bias` | ✅ `jfet_id_vds` | pending | ✅ `noise_jfet` (analytic gm, nlev=3 — T12) | pending | pending | `ic` UIC declared but unwired (T17) |
| bjt | `bjt.phdl` | ✅ `bjt_ce` | pending | pending | pending | pending | ✅ `temp_bjt` (VAF/VAR temp — T15) | SPICE2-era temperature model only (~50 modern temp-coeff params absent); excess phase (PTF) dropped (defaults to 0 → exact); `ic` UIC declared but unwired (T17) |
| mos1 | `mos.phdl` | ✅ `nmos_fixed` | ✅ `nmos_id_vgs` | pending | ✅ `noise_mos_linear` (analytic gm — T11) | pending | pending | flicker `cox=0` substitute landed (T14); mos3's `badMos3` alt mode intentionally absent; `ic` UIC declared but unwired (T17) |
| mos2 | `mos.phdl` | ✅ `nmos2_fixed` | ✅ `nmos2_id_vds` | pending | ✅ `noise_mos2_linear` | pending | pending | as mos1; `.disto` blocked (Cranelift function-count — P1 backlog) |
| mos3 | `mos.phdl` | ✅ `nmos3_fixed` | ✅ `nmos3_id_vds` | pending | ✅ `noise_mos3_linear` | pending | pending | as mos1; `.disto` blocked (P1 backlog) |
| tline | `tline.phdl` | pending (in-process `tests/tline.rs` only, not vs ngspice) | pending | pending | n/a (lossless, noiseless) | pending | pending | `FREQ/NL` delay spec missing; IC params missing |
| urc | `urc.phdl` | ✅ `urc_lump2` | pending | ✅ `urc_taper` (geometric K taper — T19) | pending | pending | pending | ISPERL diode-mode shunt landed (T20); **NGP-18/19/22 documented deviations** — generic `urc[N]` authored but the codegen side does not yet lift array-wire elements or process structural `connections`, so `FMAX → N` auto-select and retirement of the hand-unrolled triple wait on that integration |

**Cross-cutting issues (status updated from the 2026-07-28 audit):**

1. **Channel/gm noise in MOSFETs/JFETs.** **Closed** by T11/T12 — now uses
   the analytic `gm`, golden-covered in the linear region (`noise_mos_linear`,
   `noise_mos2_linear`, `noise_mos3_linear`, `noise_jfet`).
2. **`ic`/UIC initial conditions.** The 2026-07-28 audit's *"`@initial`
   branch-forcing in the JIT"* blocker claim was **stale** — the mechanism
   (`flatten/analog.rs:876` collecting `@initial { V(p,n) <- ic; }` as UIC
   seeds, `analyses/transient.rs:228`'s hold clamps) landed and works. What
   remains is ordinary model-wiring work: T16 (cap/ind/diode) and T17
   (bjt/jfet/mos1/2/3) are pending in `ngspice-parity`. A separate open:
   `cap`'s default `ic=0` always-clamp is a distinct, narrower issue from
   the (now-closed) JIT-mechanism gap.
3. **Source-stepping homotopy (`CKTsrcFact`).** **Closed** by T18 — the
   audit's negative was a false alarm. `vsrc`'s forced value is scaled on
   its MNA force row; `isrc`'s DC value scales through
   `$simparam("sourceScaleFactor")` in its body, and the TRANOP runs as a
   DC-analysis solve so the non-`tran` branch (carrying the scale) is what
   a transient's t=0 OP takes. The falsifiable proof is the new
   `homotopy_bias` and `homotopy_sink` fixtures — neither alone falsifies
   both, so two fixtures ship (`tests/ngspice_homotopy.rs`).
4. **No behavioral/arbitrary source (`asrc`/`B` element).** **Closed by
   rejection** (MD-39): the `B` element exists because a SPICE netlist has
   no expression language; PHDL does — `V(p,n) <+ <expr>` in an `analog`
   block *is* the behavioural source, differentiated symbolically. An
   imported deck's `B` line is transcribed into a PHDL body by the
   SPICE→PHDL importer (Part VI §9). Porting `asrc` as a native device
   would reimplement the language's own premise as a model.

**Missing ngspice devices (no piperine model at all):**

| Category | ngspice devices | Status |
|---|---|---|
| **BSIM family** (compact MOS for deep-submicron) | `bsim3` (BSIM3v3.3), `bsim4` (BSIM4.8) | **Split into their own features** — `.specs/features/ngspice-bsim3/` (port-first; ~11.4k LOC C, ~166 params) and `.specs/features/ngspice-bsim4/` (~22.6k LOC C, ~236 params — the largest single item in the model backlog). Both are scaffolded `NOT YET SPECIFIED`; both depend on this feature's P1 oracle and P4 (`gap-3`). The `v0`/`v1`/`v32`/`v5`/`v6`/`v7` legacy forks are out of scope (frozen pinned model cards). |
| **Advanced BJT** | `vbic` | **Rejected on merit** (MD-39). Foundry-grade RF BJT outside the north-star user (hobbyists, independents, small teams without Cadence-class tooling); the Gummel-Poon `bjt` already serves that user. Revisit only on measured demand. Also `nbjt`/`nbjt2` (1D Poisson solvers) — different engine, not a compact model. |
| **Compound-semiconductor FETs** | `jfet2`, `mes`/`mesa`, `hfet1`/`hfet2` | Medium priority, deferred pending the BSIM ports proving the porting method scales. |
| **Surface-potential MOS** | `hisim2`, `hisimhv1` | Low — niche academic/foundry models. |
| **Niche MOS** | `mos6`, `mos9` | Low — rarely used. |
| **Lossy transmission lines** | `ltra` (RLGC convolution), `txl` (exponential), `cpl` (coupled multi-conductor) | Medium — `urc` covers the practical distributed-RC case; `ltra`/`txl`/`cpl` are their own feature if scheduled. |
| **Numerical / 1D solvers** | `numd`, `numd2`, `numos` | Low — academic Poisson solvers, a different engine. |
| **Verilog-A / shared-lib devices** | `adms`, `ndev` | Out of scope for the solver core — served by the external OSDI/plugin path (`piperine-osdi`). |
| **Behavioral source** | `asrc` (`B` element) | **Rejected on merit** (MD-39) — see Cross-cutting issue #4 above. PHDL's `analog` block is the behavioural source; an importer transcribes decks. |

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

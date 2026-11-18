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
| DV — Design verification | **constraint core re-anchored on VAMS** (2026-08-17, `constraints-on-vams`); gradients/tester partially dark — see below |

**Delivered in V1 so far (~1360 tests, ngspice cross-validated):** every
analysis a SPICE user expects (DC/AC/tran/noise/PSS/sens/tf/pz/disto/sp/four),
TR-BDF2 transient with gmin/source stepping, Element ABI with rollback +
limiting + lifecycle, `import piperine` host with typed configs/results/sweeps,
LSP with scope-aware resolution + doc comments, native+Python plugin surface,
and the constraint core (`constraint begin` blocks, margins, findings,
postures).

---

## Forward-looking items

### P1 backlog (not V1-blocking)

| Item | Status |
|------|--------|
| `.disto` on MOS2/MOS3 — Cranelift function-count explosion | **open** — kernels are opt-in (`disto(true)`, default off); fix = stop emitting the ordered cross-product (combinations are symmetric → one indexed function, not N) |
| A device transient with a non-zero start and no captured step | **open** — fails loud (digital init only at tick 0): `TransientAnalysisOptions::with_start` needs `with_initial_state`; found in `solver-abi-integration` (T18), no feature owns it yet |
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
| cap | `passives.phdl` | n/a (open at DC) | n/a | ✅ `ac_rc` | ✅ `noise_rc` | ✅ `tran_rc` + ✅ `uic_rc` (T16) | pending: no temp sweep | — |
| ind | `passives.phdl` | pending | pending | pending | n/a (reactive, noiseless) | pending | pending | `ic` UIC **documented deviation** (T16): ngspice's `ind.IC` is a **current** seed (`indload.c` flux `L·I`); `collect_initial_conditions` only collects `Potential` seeds — a current-seed mechanism is a separate codegen feature |
| sw | `switches.phdl` | pending | pending | pending | ✅ `noise_switch` | pending | pending | ON-state thermal noise landed (T13); negative hysteresis approximately right |
| csw | `switches.phdl` | pending | pending | pending | ✅ `noise_csw` | pending | pending | ON-state thermal noise landed (T13) |
| dio | `diode.phdl` | ✅ `rdiode` | ✅ `diode_iv` | pending | pending | ✅ `uic_rc` (T16) | ✅ `temp_diode` | LEVEL=3 geometry path (w/l→area) missing |
| jfet | `jfet.phdl` | ✅ `jfet_bias` | ✅ `jfet_id_vds` | pending | ✅ `noise_jfet` (analytic gm, nlev=3 — T12) | pending | pending | — |
| bjt | `bjt.phdl` | ✅ `bjt_ce` | pending | pending | pending | pending | ✅ `temp_bjt` (VAF/VAR temp — T15) | SPICE2-era temperature model only (~50 modern temp-coeff params absent); excess phase (PTF) dropped (defaults to 0 → exact) |
| mos1 | `mos.phdl` | ✅ `nmos_fixed` | ✅ `nmos_id_vgs` | pending | ✅ `noise_mos_linear` (analytic gm — T11) | ✅ `uic_mos` (T17) | pending | flicker `cox=0` substitute landed (T14); mos3's `badMos3` alt mode intentionally absent |
| mos2 | `mos.phdl` | ✅ `nmos2_fixed` | ✅ `nmos2_id_vds` | pending | ✅ `noise_mos2_linear` | pending | pending | as mos1; `.disto` blocked (Cranelift function-count — P1 backlog) |
| mos3 | `mos.phdl` | ✅ `nmos3_fixed` | ✅ `nmos3_id_vds` | pending | ✅ `noise_mos3_linear` | pending | pending | as mos1; `.disto` blocked (P1 backlog) |
| tline | `tline.phdl` | pending (in-process `tests/tline.rs` only, not vs ngspice) | pending | pending | n/a (lossless, noiseless) | pending | pending | `FREQ/NL` delay spec missing; IC params missing |
| urc | `urc.phdl` | ✅ `urc_lump2` | pending | ✅ `urc_taper` (geometric K taper — T19) | pending | pending | pending | ISPERL diode-mode shunt landed (T20); generic `urc[N]` retired the hand-unrolled triple (T22) once gap-3's codegen side landed (Fix A `ba79501` array-wire element registration + Fix B `8a8114b` structural-connection unification); `FMAX → N` auto-select via `URC_DEFAULT_N` (T23). Compile-time taper propagation remains a SPEC_DEVIATION (`urc.phdl:41-49`) — per-instance `k` override needs runtime staging |

**Cross-cutting issues (status updated from the 2026-07-28 audit):**

1. **Channel/gm noise in MOSFETs/JFETs.** **Closed** by T11/T12 — now uses
   the analytic `gm`, golden-covered in the linear region (`noise_mos_linear`,
   `noise_mos2_linear`, `noise_mos3_linear`, `noise_jfet`).
2. **`ic`/UIC initial conditions.** The 2026-07-28 audit's *"`@initial`
   branch-forcing in the JIT"* blocker claim was **stale** — the mechanism
   (`flatten/analog.rs:876` collecting `@initial { V(p,n) <- ic; }` as UIC
   seeds, `analyses/transient.rs:228`'s hold clamps) landed and works. T16
   closed the cap + diode wiring and the narrow codegen gap that had kept
   the wiring global-off (per-seed param-dependency mask in
   `AnalogInstance::initial_conditions` suppresses a seed whose backing
   params are all at declared defaults, so the model default `ic=0` does
   not clamp every transient to zero); T17 closed the bjt + jfet + mos1/2/3
   wiring (bjt: `ic_vbe`/`ic_vce`, jfet/mos: `ic_vds`/`ic_vgs`). The UIC
   one-step hold is released right after the DC operating point so the
   first integration step is natural (matching ngspice's `CKTsetIC`).
   **Two documented deviations remain:** (a) `ind` — ngspice's `ind.IC` is
   a **current** seed (`indload.c` flux form `L·I`), but
   `collect_initial_conditions` only collects `Potential` seeds — a
   current-seed mechanism is a separate codegen feature; (b) mos/bjt/jfet
   — piperine's `@initial` pins the external node, whereas ngspice seeds
   the device's internal state during UIC init. The two agree with no
   series resistance (`uic_mos`'s case) and diverge only when RD/RS are
   nonzero (a future device-state-seed codegen feature).
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

**Constraint surface ✅ re-anchored on Verilog-AMS** (2026-08-17,
`constraints-on-vams` MD-49 + `constraint-statements`). The PHDL-era
dv-core surface went dark at the frontend swap (MD-44/45); the full
constraint statement surface is back, as a sanctioned language extension
rather than the old PHDL grammar:

- `constraint begin … end` — a module-body block with contextual
  keywords (Annex B's reserved set is untouched): `require`, `var`,
  `target` (soft goals, `target#<n>`), `cover`/`cover cross` (bin
  coverage merged across runs), and reduced AC metrics
  (`ac_unity_gain_freq`/`ac_phase_margin`/`ac_gain_margin` as
  host-computed margins)
- Signed margins (`m ≥ 0` ⟺ satisfied), `tol`-normalized, worst + argmin
  (time/freq/sweep + instance path) through the `MarginsResult` channel
- Evaluation at accepted solution points only (LRM §9.7.3's
  rejected-iteration rule inherited: a violation inside a rejected Newton
  iteration is never reported)
- Three postures: `strict` (default, `$error` — aborts), `collect`
  (`$warning` — records), `off` (zero cost)
- `validation_reports()` + `EMITS_VALIDATION` findings channel — preserved
  end to end, now fed by a real producer
- `ConstraintCatalog` classifies a constraint the running analysis cannot
  provide (digital instance) — reads fail loud naming it

**Nothing left dark from the PHDL era** — the full dv-core surface is
back on the VAMS producer: constraint statements (`require`/`var`/
`target`/`cover`/`cover cross`), analysis scoping (`@ dc`/`@ tran`/
`@ ac`) with temporal windows, reduced AC metrics, SOA blocks on the
spice stdlib models (absent-by-default sentinels), the `tol gauss/
uniform` catalogue with global/mismatch scoping and (seed, index)
replay, UIC seeds (`@(initial_step) V(a,b) <+ value`), the margin-
gradient classifier, and the `rfport` schema behind `.sp`.

---

## P7 — Optimizer (design centering)

**Target:** maximize yield over process/tolerance spread. A user declares
random variables on params, writes `constraint begin` blocks for specs, and
the optimizer centers the design so every margin stays positive across the
declared variation.

**Foundations in place (constraint core + host-library):**

| Foundation | Status |
|---|---|
| Compile-once restamp sweeps (MD-18) | ✅ `Session::sweep`/`sweep_grid` |
| Live `Session::set` (restamp, no re-JIT) | ✅ Rust host |
| `.sens` (DC sensitivity, finite-difference) | ✅ both hosts |
| Signed margins with argmin (the objective) | ✅ `constraints-on-vams` `MarginsResult` |
| `collect` posture (records worst margin) | ✅ both hosts |
| `Param.bounds` (auto knob-limits) | ✅ host-library |

**Missing before this phase hardens (ordering matters):**

- [ ] **The random-variable catalogue** — first-class on the POM, not a
      per-param vestige: correlation, global-vs-mismatch scoping, stable
      replay index, and compile-once N-sample runs are all cross-parameter
      facts. Its shape must be settled **before** optimizer work hardens
      around a per-parameter view (`constraints-on-vams/context.md`
      records the direction: re-anchor on the LRM §9.13.2 `$rdist_*`/
      `$random` functions piperine already implements).
- [ ] **Algorithm family** — worst-case distance vs Monte-Carlo yield vs
      ellipsoidal. No decision forced yet.
- [ ] **`∂F/∂p` analytic kernel** — the gradient feed the optimizer reads.
      Today only finite-difference `.sens` exists; the analytic path is
      faster and feeds centering directly. With random variables
      catalogued, `∂margin/∂(random var)` becomes available too — "yield
      is 92%, dominated by M1's Vth mismatch".
- [ ] **Monte Carlo / corner sampling** — sample the catalogue, sweep each
      sample through `Session::sweep`, collect margins.
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

- Refactor `crates/piperine-lang/src/pom/{resolve,reflect,lower}/mod.rs` into named modules after msl-stdlib merges (registered hygiene exemptions in `crates/piperine-lang/tests/suite_hygiene.rs`).

## Solver ABI follow-ups (from solver-abi-integration verification round 2, 2026-11-15)

- **Re-entry is approximate, not a replay (N1, Low).** Re-entering a transient at a captured clock-edge step lands on a different step grid and ends up to 7.3e-4 relative off the continuous run. Make re-entry restore the step controller state so it replays the continuous run bit for bit.
- **Re-entry state on every step (N2, Low).** Every accepted step stores the full re-entry state and `mark()` copies all net bytes on each attempt; memory and time grow with step count. Add an opt-out (or capture only at host-requested points).
- **Partials guard matches by label (N3, Low).** The C27 guard ties a device to its check by label string, not by the module verified; tie it to the module.

## M1 follow-ups

- **A child's own signals show as `gnd` in the authored `Child` (msl-elab).** `Child::nodes` is padded with `Ground` for each signal of the placed module because `Terminal` cannot name the child's own node; the instance table carries the real one. Needs a `Terminal::Own` in `piperine-ir` (and `Ir::check` to accept it). No M1 device has a signal.

## MSL spec gaps (spec text owed)

Found by the buck prototype (`examples/buck/GAPS.md`). Each has a provisional rule so work is not blocked; the spec sentence is still owed.

- **G4 - float to `UInt<N>`/`Int<N>` with `as`.** Provisional (ir-rust): saturate to the N-bit range, NaN to 0. Spec Part I §1 rule 1 says "truncates or extends, as in Rust", which is ambiguous for non-Rust widths.
- **G5 - shift by N or more.** Provisional (ir-rust): mathematical result truncated to N bits (0 for `<<` and unsigned `>>`, sign fill for signed `>>`). No spec rule. A shift by a negative amount (an `Int<M>` amount below 0) has no rule either: the compiled code panics naming the shift, which the solver reports as a fault (ir-rust phase-B verification F-B9).
- **G6 - unknown integers are per value, not per bit.** Provisional (ir-rust F-B4, orchestrator ruling 2026-11-15, user may revisit): one known flag per value, the ABI's one known byte; §3.3.2's `&`/`|` shortcuts apply only when one operand is all-zeros/all-ones. Partial masks give unknown where §3.3.2 per-bit gives known bits (`(x & 1) & 2` with `x` unknown). Per-level flags would change the ABI.
- **G1 fixed-point type, G2 ranged integer.** Language proposals, not bugs; schedule on demand.

## Queue after ir-rust (user, 2026-11-15)

User, verbatim: "Depois desses desenvolvimentos do ir -> transpilation -> abi eu preciso que tu coloque na fila acertar o CLI, project para poder rodar algo completo. Aí em seguida gostaria de e2e test fixtures, com paridade ngspice. [...] Aí deixamos os e2e em um lugar só, no root, para testar um monte de cenários".

1. ✅ **Delivered (2026-11-15, `feature/ir-rust`).** **`ir-rust`** (M3): phase A analog, then phase B digital/mixed buck slice.
2. ✅ **Delivered (2026-11-16, `feature/msl-digital`).** **`msl-digital` — digital and mixed MSL through the front end** (user, 2026-11-15: "Sim, por favor! Achei que já tivesse no escopo completo"). Type check and elaboration of the digital and mixed forms (definitions, carriers, drives, events, reactions, commits, `UInt<N>`/`Int<N>`, unknown values, `cross`/sampling at the boundary) into the IR digital node set `ir-rust` phase B defines, so **`examples/buck` runs from its `.msl`** on the transpiled backend, closed loop included. Pulled forward from M2. Built on `feature/msl-digital` (67 tasks in 10 phases); what it leaves is in `GAPS.md`.
3. ✅ **Delivered (2026-11-17, `feature/msl-cli`, 49 tasks; the Book's command-line chapter at a7b6429d).** **CLI + project, run something complete** (`msl-cli`; `piperine check`/`build`/`run`/`test`/`new`/`clean`/`fmt` on an MSL project, `piperine-api` rewritten small, every CLI listing of the Book held to the binary; what it leaves is in `GAPS.md`; the language server is out of it and deferred, user 2026-11-16: "O lang-server é o menos importante, vai ficar para bem depois"). `piperine-cli` and `piperine-project` are back in the default build and speak MSL: `Piperine.toml` discovery, `piperine check` / `build` / `run` on an MSL project (e.g. `examples/buck`, the ngspice twins), backend choice (interpreter or transpiled), analyses and results out of the CLI. Today both crates are outside `default-members`.
3b. **Spec-completeness planning pass (after the CLI).** User, 2026-11-15: "no fim do cli, recomendo passar um fable para pegar o spec completness para planejar as outras features. Antes de mandar essa, me avisa dos gaps". A Fable reads `docs/spec/` against the implementation and plans the remaining features. **Before dispatching it, the orchestrator presents the current gaps (`GAPS.md`) to the user.**
3c. ✅ **Delivered (2026-11-17, `feature/long-run`, 23 tasks).** **`long-run` — a run that observes instead of keeping.** A transient hands each accepted step to a host observer and a host keeps rows, never steps: a session records only the signals it prints, on the `print_step` grid as the steps arrive, so memory depends on the rows and not on the run's length; `examples/buck`'s `startup` prints the whole 600 µs start and runs through the host library on a 10 ns grid (60 001 rows, every check passing, overshoot, settling and ripple measured against its SPEC). What it leaves, in the order the user set (decision 6):
    - **Multi-rate stepping** - digital edges that do not cut analog steps (the analog side still lands near every 200 MHz instant, so the whole start takes about 130 s in a dev build).
    - **Surrogate or averaged models** - a converter whose switching is averaged, for runs longer than the start.
    - **Periodic steady state** - on the device ABI; the next feature.
4. **Root wiring + integrated tests** (user, 2026-11-16: "Finalizando o cli + api na feature, pode fazer o wiring do root e criar os testes integrados. Eu recomendo que tudo que está no livro tenha ao menos 1 teste integrado"). Root package `piperine` re-exports the new `piperine-api` and holds the integrated tests. **Rule: every example and behavioral claim in the Book has at least one integrated test** that runs it through the user's entry point (api/CLI); a Book example that cannot run yet fails loud and is listed in `GAPS.md`, never silently untested.
4b. **Root e2e fixtures with ngspice parity.** One place, root `tests/e2e/`: many scenarios, each an MSL project (or file) + its `.cir` twin + expected outputs, run through the same entry point a user runs (the CLI / project path), compared with ngspice through the single `ToleranceContract`. The current `tests/ngspice/` twins move under it; `piperine-parity` stays the harness.
5. **M2 remainder** - galleries E/F and conformance C.2.3 on the transpiler path, and the stdlib digital/mixed modules the buck does not use (`msl-digital` already brings `transition`, `A2D`, `above`, `init`, `Never`, `log`, `require`, enums/`match`/`Option`, timers).


## Every gap has a feature (user, 2026-11-17)

User, verbatim: "gostaria que abrisse feature e escrevesse a spec para todos esses gaps, pode dividir da maneira que achar legal pois eles todos são super importantes. Não precisa avançar para o design mas deixa aqui as features abertas e coloca no roadmap. Preciso que TODOS os gaps estejam endereçados ok? Principalmente esses que tu listou que limitam a expressividade da linguagem ok?"

Every id of `GAPS.md` (audit of 2026-11-17) has one owning feature. Specs only for the open ones (`.specs/features/<name>/spec.md`), each with a "Gaps closed" table. All 77 open questions were answered with their recommended option (user, 2026-11-18: "Sobre as open questions, vou de aceito todas ok?"); the specs are approved. Design and tasks come when a feature is picked.

**In flight (three lanes, user 2026-11-17 "Topo"):**

| Lane | Feature | Gaps | State |
| --- | --- | --- | --- |
| A | `msl-fixes` | S1, S5, B1, B2, L11, L14, L15, L16 (H.1–H.4), L17, L41, L46, L47 (`freq`), L48, L54, L55 | implementing |
| B | `host-ii` | S2, X3, X4, X7, X8, X9, H2 (tf, pz, sens, pss), H3, H4, H5, H9, L56, `max_steps` | implementing |
| C | `solver-hygiene` | V1–V11, V14, V15 | implementing |
| B next | `root-e2e` | H11, U5, U10 | spec approved |
| B next | `buck-scenarios` | U3 (SPEC S2–S9 as runs + host tests; load tests at 3.3 V) | queued, after host-ii |

**Language expressiveness (the user's priority):**

| Feature | Gaps | Size | Waits on |
| --- | --- | --- | --- |
| `analog-structure` | L26, L27, L34–L38, L40, L42, L50, S4 | L (~30–35) | msl-fixes; host-ii (`set_at`, X9); generics and params-and-io for the stdlib `idt`, `Water`, §18 `Diode` |
| `analog-operators` | L43, L44, L45, L47 (`simparam`, `tnom`), L49, H2 (noise analysis), U7 | L (~25) | msl-fixes; host-ii; analog-structure (AS-07) |
| `composite-values` | L1–L6, L9, L10, L13, L16 (H.5–H.7, `normal`), L24, the enum variant names | L (~95, phases A–D) | msl-fixes; generics A for `Fn`; unknown-semantics for unknown loops |
| `generics` | L4 (generic enums), L7, L8, L29–L32, L58 (generics group), S3, X6 | L (~114, phases A–E) | composite-values A–C; order CV-A, CV-B, GN-A, GN-B, CV-C, GN-C, GN-D, GN-E |
| `unknown-semantics` | L18, L19 | L (~26) | msl-fixes S1 |
| `unknown-semantics-events` | L51, L52, L53, `limexp`, the Book's clipper and chapter-8 buck | M (~17) | unknown-semantics; generics L8 |
| `unknown-semantics-per-level` | S6, G6 (re-opens the 2026-11-15 ruling) | S–L by the answer | unknown-semantics |
| `resolved-nets` | L21, L22, L23, L58 (resolved-net group), the Book's alarm | L (~30, 3 phases) | phase 1 none; then composite-values, unknown-semantics, generics |
| `resolved-nets-bundles` | L25 | M (~15) | composite-values, generics, resolved-nets |
| `params-and-io` | L20, L28, L39, L57, X5, H12, the GAPS §10 code guard | L (~30) | composite-values (arrays); host-ii |
| `foreign-models` | L12, L33 | L (~35) | host-ii; generics (`m`/`dtemp`); decisions on OSDI and linking |
| `spec-closure` | G1, G2, G4, G5, X1, X2, H13 | S–M (10–22) | solver-hygiene (lands after it) |

**Host and use:**

| Feature | Gaps | Size | Waits on |
| --- | --- | --- | --- |
| `host-iii` | H1 (force, release, waits), H6 | L (~32) | host-ii; solver-hygiene (exact re-entry) |
| `host-analyses` | H2 (S-parameters, Fourier, distortion), op variables | M (~24) | host-ii |
| `sweeps-montecarlo-optimization` | H2 (multi-axis sweeps), H7 | L (~34) | host-ii |
| `gradients` | H8, E0807–E0809 | L (~40) | host-ii; solver-hygiene |
| `cli-packages` | H10 | M (~26) | piperine-project |
| `python-host` | B4 | L (~35), **deferred to after the host features** (user 2026-11-18: "podemos fazer tudo em RUST por enquanto e DEPOIS pensamos em integrar python") | piperine-api; host-ii; host-iii and siblings |
| `lang-server` | B3 | M (~20), low priority (user) | — |

**Performance and proof:**

| Feature | Gaps | Size | Waits on |
| --- | --- | --- | --- |
| `multi-rate` | V12 | L (~14), later (user) | solver-hygiene; host-ii `max_steps` |
| `surrogate-models` | V13 | M (~12), later (user) | generics; params-and-io; multi-rate |
| `periodic-small-signal` | V16 | L (~20) | host-ii (pss); analog-operators (noise) |
| `conformance` | U1, U2, U4, U6, U8, U9 | L (~55, 6 phases; 1–5 can start now) | per phase: generics, composite-values, resolved-nets, analog-*, host-analyses (Fourier) |

Ownership notes: Fourier and distortion belong to `host-analyses` (conformance's `four_diode`/`disto_diode` wait on it); V6 stays with `solver-hygiene`; `E0361` is raised first by `params-and-io`.

## Execution order (user, 2026-11-18)

User, verbatim: "Eu gostaria que engatilhasse até o 12. Depois do 12 insere um refinamento no meio somente para refactor de visibilidade de código e estrutura interna. O refinamento tem que ser feito depois do fechamento da 12 pois vamos ter código novo entrando sempre e aí depois do refactor entra o 13 com a verificação de cosure da spec usando o Fable e desenvolvimento dos gaps finais."

Dispatched without asking, up to three lanes at a time (implementation Sonnet, verification Opus, merge to main and upstream on close):

1. `msl-fixes` close · 2. `host-ii` close (rebased on msl-fixes) · 3. `solver-hygiene` close
4. MD-number renumbering at merge where lanes collide
5. `root-e2e` · 6. `buck-scenarios`
7. `analog-structure` · 8. `unknown-semantics`
9. `composite-values` phases A–B, then `generics` phases A–B
10. `analog-operators`, `unknown-semantics-events`, `params-and-io`
11. `composite-values` phase C, `generics` phases C–E, `unknown-semantics-per-level`
12. `resolved-nets`, `resolved-nets-bundles`

**Then, after item 12 closes and before anything else lands code:** `code-clarity` — a refactor for readability and internal structure under the user's seven cleanliness rules (`.specs/features/code-clarity/spec.md`). Its full findings inventory is written first by a Fable reading the whole codebase.

**Then item 13:** `spec-closure` with a Fable verification of spec closure (spec ↔ implementation), and the development of the final gaps (`host-iii`, `host-analyses`, `sweeps-montecarlo-optimization`, `gradients`, `cli-packages`, `conformance`, `multi-rate`, `surrogate-models`, `periodic-small-signal`, `foreign-models`; `python-host` and `lang-server` last).

**Item 13 is also dispatched without asking. The automatic run stops after item 13** (user, 2026-11-18: "a gnt para o run automatico e engatilhado pós 13, aí eu vou avaliar o closure, spec, código para levantar nosso backlog de melhorias"): the user then reviews the spec closure, the spec and the code to build the improvement backlog.

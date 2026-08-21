# Project State — Piperine

## Macro Decisions (locked)

These are the architectural decisions that shape the solver. They are binding
and won't be relitigated per-PR. Feature specs reference them by ID.

### MD-01: One Element ABI, no downcast
The solver talks to every participant through one `Element` trait with
`ElementCapabilities` bitflags. No `Device` wrapper, no analog/digital facet
split, no downcast. A resistor, a gate, a comparator, and a plugin are the
same type to the solver.

**Status:** Done (amended 2026-07-19, solver-simplification batch 3):
`Element` is the conjunction of concern-scoped supertraits (`AnalogDevice`
+ `DigitalDevice` + `Introspect`). The object is not split — still one
`Element` ABI, still no `Any`/downcast; only its surface is grouped so
each concern is separately legible, and capability flags still gate which
concern runs. Rationale beyond legibility: a downcast-based facet split
would block the future C-style FFI ABI — supertraits keep the object flat
across the boundary.

### MD-02: Net is the unified naming layer
`Net` (kind + dense index + label + optional `Arc<AnalogVariable>`) replaces
both `AnalogReference` and `DigitalNet` at the public boundary. Fast-path
aliases remain for hot loops. Result types answer `get_net(&Net)`.

**Status:** Done.

### MD-03: Per-analysis context, shared Context
`Context` carries only what every analysis shares (tolerances, integration
method, temperature, verbosity). Each analysis receives its own
`AnalysisContext` (`DcContext`, `AcContext`, `TransientContext`, etc.) with
analysis-specific tunables (`dt_min`, `dt_max`, `adaptive`, sweep config, …).

**Status:** Locked. Implementation pending.

### MD-04: Tolerances vs Policy
`Context` holds immutable `Tolerances`. Mutable `Policy` (homotopy scales,
step bounds, retry counters) is owned by the active `ConvergencePlan` and
its strategies — never by the shared `Context`.

**Status:** Done (solver-convergence-performance T11). `Context` is
tolerances-only; `Policy` lives on each analysis solver; time is explicit.

### MD-05: Strategy composition
The analysis state machine (setup→op→resume→accepted→rejected→restart) is
composed of three strategy traits: `NewtonStrategy`, `HomotopyStrategy`,
`StepperStrategy`. Each analysis picks the strategies it needs. No inline
if-else cascades in drivers.

**Status:** Done (2026-07-19, solver-simplification). `HomotopyStrategy`
(gmin/source stepping), `NewtonStrategy` (`DampedNewton`, wired in
`analyses/dc.rs` + the transient kernel), and `StepperStrategy`
(`PiController`, wired in `analyses/transient.rs`) all shipped and wired;
homotopy/stepper literals live in `analyses/config.rs`.

### MD-06: init_global as Once
`tracing`/`faer` need one-time process init. `Context::default` does not
trigger it; `Solver::build()` does.

**Status:** Locked. Implementation pending.

### MD-07: Integration helpers in math/
`TrBdf2`/`TrBdf2Phase`, companion coefficients (`phase_coeffs`/`stage_coeffs`),
the Milne LTE estimate and `Integrator` (quadrature) all live in
`math/integration.rs`. The kernel calls the centralised formula — no
per-method branching in codegen. The vestigial `IntegrationMethod` enum, the
dead `TruncationError` trait and `Tolerances.integration` were removed
2026-07-18 (p1-solver-complete T22): TR-BDF2 is the sole scheme, there is no
method-selection surface.

**Status:** Done (amended 2026-07-18).

### MD-08: LTE drives timestep
After each accepted step, the stepper consults elements for LTE-based dt
suggestions. Takes the min, clamps to `[dt_min, dt_max]`. Non-reactive
circuits fall back to 2× growth. No allocation on hot path.

**Status:** Done.

### MD-09: SolverDomain enum
Error domain is a typed enum, not a free string. Typos are compile errors.

**Status:** Done.

### MD-10: Scheduler returns Result
Digital scheduler returns `Result<(), Error>` instead of `log::warn!`.
Caps live in `PlanLimits`.

**Status:** Done.

### MD-11: OSDI as checklist, not ABI
OSDI is a maturity checklist. Piperine's ABI is mixed-signal-first and
domain-uniform. OSDI wrappers are one client.

**Status:** Locked.

### MD-12: ABI vs solver policy classification
Element "is" or "knows" → ABI. Solver "decides" → solver policy. Per-device
convergence tests stay in ABI (element volunteers); solver gates the outer
loop on global convergence.

**Status:** Locked.

### MD-13: Rust idiom rules (binding)

These five rules govern every line of solver and codegen code. A PR that
violates any of them is not ready. They are also in `AGENTS.md` under
"Hard rules → Rust idiom rules".

1. **Contracts and capabilities first.** Think in traits, capability
   descriptors, and type-level contracts before algorithms and
   implementation. The code should read as a specification of *what* the
   solver does, not *how* it does it internally.

2. **No loose functions.** Every function has an owner — a trait method or a
   struct method. `pub(crate) fn` or `pub fn` at module level is a defect.
   If a helper doesn't belong to a trait or struct, it means the abstraction
   is missing.

3. **Clean and simple.** Bat the eye and understand what the code is doing.
   If a reader needs to trace three files to understand a single operation,
   the code is too clever. Prefer explicit over implicit, flat over nested,
   early-return over deep match.

4. **Modules organized by system function.** Files are named after what they
   do in the system (`solver.rs`, `integration.rs`, `circuit.rs`), not after
   language constructs (`traits.rs`, `models.rs`, `utils.rs`). The golden
   rule: glance at the file tree and know where every struct and trait
   belongs.

5. **No macros.** No `macro_rules!`, no `paste!`, no proc-macro codegen.
   Data tables + plain helpers. If a pattern repeats, extract a trait or a
   struct method — never a macro.

**Status:** Locked. Enforced in AGENTS.md.

### MD-14: TF voltage-source-only
TF keeps explicit error for current-source input. Documented limit, not a
gap.

**Status:** Done.

### MD-15: No piperine-math crate
The math dispatch table was absorbed into `piperine-lang` / `piperine-codegen`
directly. There is no standalone `piperine-math` crate in the workspace.

**Status:** Done.

### MD-16: Crate-level docs removed
Per-crate documentation (`crates/*/docs/`) was removed. The formal spec lives
in `docs/spec/` (Parts I–VII). Solver gaps and feature tracking live in
`SOLVER_GAPS.md` and `.specs/`.

**Status:** Done.

### MD-17: Two-tier public surface — `prelude` + `abi`

Delivered by `solver-abi` feature. Public surface of `piperine-solver` is
exactly two modules: `prelude` (host audience: bench, python, CLI) and `abi`
(device-author audience: codegen, plugins, test doubles). Every other module
is `pub(crate)`. Re-exports in `prelude`/`abi` elevate item visibility without
reopening internal module paths.

- `prelude`: `CircuitBuilder`, `CircuitInstance`, `Solver`, `Context`, `Policy`,
  `Tolerances`, analysis results + options, `Net`, `Error`, `SolverStats`, …
- `abi`: `Element`, `ElementCapabilities`, `UnknownAllocator`, `Stamp`,
  `AnalogReference`, `Netlist`, `Noise`, `NoiseKind`, digital interface, …
- `lib.rs`: `pub mod abi; pub mod prelude;`, all others `pub(crate)`.

**Status:** Done (2026-07-16).

### MD-18: Elaboration fixes devices; simulation never re-JITs

Elaboration/JIT happens once per design+staging; after that, a simulation —
including parameter sweeps — runs entirely on the solver. Re-running
elaborate/compile inside a simulation loop (e.g. per sweep point) is an
architecture defect, not a perf tweak. Swept parameters go through a
solver-level restamp/staging path on the already-compiled circuit.

**Status:** Locked (user, 2026-07-16). Implementation: spice-stdlib T12.

### MD-19: Root crate is the library face (lib-only; bin in cli)

The root `piperine` crate is the complete external Rust view of the project:
`src/lib.rs` hosts the session/results/waveform plumbing plus a `prelude`
re-exporting the lang/codegen/solver public faces. The root is **lib-only** —
the `piperine` binary target lives in `piperine-cli` (`[[bin]] name =
"piperine"`) because root(bin)→cli→python→root(lib) would close a cargo
package cycle. Dependency flow: `root(lib) → {lang, codegen, solver}`;
`python → root(lib)`; `cli → {python, root(lib), project}` + bin.
`cargo install` targets `crates/piperine-cli`.

**Status:** Locked (user, 2026-07-17 — bench-removal topology option B).
Implementation: bench-removal T1. **Superseded by MD-20 (2026-07-18).**

### MD-20: `piperine-api` is the library face; root is a thin re-export shell

A dedicated `crates/piperine-api`, pure Rust: the host API
(session/results/waveform/hooks/error/prelude). `piperine-python` is a thin
binding layer over it. The root `piperine` package becomes a **thin
re-export shell** (`pub use piperine_api::*`) so Rust hosts keep
`use piperine::…` — no code of its own in root `src/`. The `piperine` binary
stays in `piperine-cli` (amended 2026-07-18: user chose re-export shell over
root-absorbs-CLI). Supersedes MD-19. Dependency flow:
`api → {lang, codegen, solver}`; `python → api`; `root(shell) → api`;
`cli → {python, api, project}` + bin — no cycle. Device/plugin ABI-contract
consolidation into the api crate is deferred to the P2/P5 features.

**Status:** Locked (user, 2026-07-18; shell amendment same day).
Implementation: feature `api-crate`.

### MD-21: Plugin backends are native + Python only

The WASM (wasmtime) and process JSON-RPC plugin tiers are removed
(`piperine-plugin-wasm` deleted with them). Native dlopen stays — trusted,
fast, and the same mechanism as the low-level `libloading` device path (V1
P2). Python plugins run through the existing embedded-host isolation (same
surface as benches); the lifecycle registry must be exposed to Python so a
plugin self-registers (attribute schemas, hooks, scripts, devices)
transparently on load.

**Status:** Locked (user, 2026-07-18). Implementation: pending (ROADMAP P5).

### MD-22: Uniform host surface — Python and Rust are one API

The two host surfaces are the same API in two languages: identical call
shape, identical names, identical config/result types. Part VIII's "two
surfaces, one surface" is **normative**, not aspirational. Concretely:
Rust gains the object model Python already has (`load` → `Design` →
`Module` → analyses / `compile()` → `LiveSession`, `InstanceView`
indexing, bundle-shaped configs); Python gains every Rust-only knob
(nodeset, `dc_damp_tolerance`, …); naming divergences (`Solver` vs
`SolverConfig`, `const_`, string-typed `cross` direction) are resolved to
one form on both sides. New analyses (sens, PSS, …) land with the same
shape on both hosts in the same feature — never one-sided. The full
Rust-side alignment is the `uniform-host-api` feature (ROADMAP P3);
Appendix C §4 is the working review sheet.

**Status:** Locked (user, 2026-07-18 — "princípio da uniformidade").
Implementation: sens/PSS bindings immediately; full alignment in P3.

---

### MD-23: Codegen module tree = pipeline stages

`piperine-codegen`'s top-level modules name the compilation pipeline stage
they perform, one module per stage: `resolve` (POM → resolved form) →
`flatten` (resolved analog → `FlatAnalog`) → `emit` (Cranelift emission
machinery) → `kernel` (compiled `AnalogKernel`/`DigitalKernel` products) →
`device` (kernels as solver `Element`s + `CircuitCompiler`). No module named
after a language construct or a vague bucket (the former `jit`/inner-`codegen`
split, which named *what tech* not *what stage*, is gone). Public surface is a
single `lib.rs` façade, not a two-tier split — codegen has one deliverable
(unlike the solver's `prelude`/`abi`, MD-17): a module stays `pub` only when
external code addresses it by deep path (grep-verified), everything else is
crate-private with host-facing items re-exported through `lib.rs`.

**Status:** Locked (user, 2026-07-20 — `codegen-architecture`).

---

### MD-24: Declared language surface — every name resolves to a textual declaration

Every referenceable PHDL name — primitive type, math function, system task,
runtime operator, attribute schema, type method — resolves to a textual
declaration in the project's headers/source (or a loaded plugin's published
`extern.phdl` stub). Name lookup that finds no textual declaration is a
compile error (`ElabErrorKind::Other`), never a silent fallback into a
Rust-native registry. A declaration marked `extern` is the only case allowed
to defer its *implementation* to a native registry — its full *shape*
(params, types, return type) is 100% textual, so LSP go-to-definition always
lands on a real declaration.

The seven `extern` forms (`extern type`/`fn`/`task`/`operator`/`attribute`/
`impl`) cover every previously-magic surface: the 7 primitive value types
(T16), libm intrinsics (T19), system tasks (T20), runtime operators (T22),
the plugin system's own `@device`/`@port` schemas (T23), and plugin-
contributed schemas via auto-imported `extern.phdl` stubs (T24/T25). Bare-
name casts (`real(x)`/`int(x)`/`bit(x)`/`Boolean(x)`/`Quad(x)`) are deleted
as a language exception (T17/T18) — replaced by ordinary overloaded
`extern impl TypeName { fn from(x: SourceType) -> TypeName; }` associated
functions, resolved by argument type (T9's overload resolution).

A permanent regression guard
(`crates/piperine-lang/tests/extern_coverage_guard.rs`, T27) iterates every
native table (`MATH_FNS`, `TaskRegistry`, the operator contract, primitive
types, schemas) and asserts a matching `extern` declaration exists — catches
the mechanism silently regressing back into "magic" after a future change.

**Status:** Locked (user, 2026-07-21 — `declared-language-surface` feature,
T1–T29 delivered). The stdlib's own native surface is fully migrated; third-
party/example plugins beyond the stdlib's own `@device`/`@port` migrate
on-demand (Out of Scope for this feature, follow-up). Binary operators
(`+`/`-`/`*`/`/`/`==`/`<`/etc.) are pure grammar (Out of Scope per spec) —
`Add`/`Sub`/`Eq`/etc. capability declarations remain user-type machinery,
not primitive-type native methods (T26's documented "none found" finding).
**Re-spelled by MD-42 (2026-08-13, `verilog-ams-support` T58):** the seven
`extern` textual-declaration forms described above do not exist in
Verilog-AMS (no Annex A production) and were deleted in full, not
retargeted, when the PHDL parser was removed (MD-45) — this is not a
syntax rename of the mechanism, the mechanism itself is gone. Only its
"no silent fallback" *principle* survives: `@device`/`@port` are the one
remaining declared-surface schema pair, re-anchored on VAMS
`(* device = ... *)`/`(* port = ... *)` attribute instances and a
name-only `ElabContext::schemas` registry (T35, VAMS-12/13) — an
unrecognized schema still fails loud. Every other previously-`extern`
surface (primitive types, math intrinsics, system tasks, runtime
operators, type-cast methods) is now a native LRM builtin the
parser/elaborator/codegen recognize directly, exactly as real Verilog-A
never needed a declaration for `sin`/`ddt`/`$display` either. See
`CLAUDE.md`'s "Declared language surface" section for the current shape.

---

### MD-25: POM navigability mirrors the source — flatten is non-destructive

POM navigability reflects the structure of the original code, never the
internal structure of elaboration. A device author reads back their own
modules, instances, and hierarchy from the POM exactly as written;
internal transforms (flattening above all) are codegen concerns they must
never have to know about.

Concretely: the `FlattenHierarchy` elaboration pass writes ONLY
`Design::flat_modules` (a `#[serde(skip)]` derived map); `Design::modules`
is never mutated. The flat form is a leaf-only, memoized, rebuildable
artifact consumed solely by codegen (`CircuitCompiler` reads the root via
`design.flat_module(root)`); the authored hierarchy in `modules` stays
navigable as written, by tools, hosts, and the LSP. This is the LOCKED
invariant every future pass that *could* collapse the module tree must
uphold — the first such pass (`FlattenHierarchy`) was the precise one the
rule was drafted against, and the audit in
`.specs/features/hierarchy-flattening/design.md` confirmed every existing
pass already honors it.

**Status:** Locked (user, 2026-07-20 — `hierarchy-flattening` feature
design review; the rule was the user's "UNBREAKABLE RULE" override that
rejected an in-place destructive flatten design). MD-13 (binding Rust
idiom rules) governs *how* the pass is written; MD-25 governs *what* it is
allowed to touch.

**VAMS clause (2026-08-13, `verilog-ams-support` T58, design.md §9):**
Verilog-AMS's own generate-block (ch. 6/Annex A `generate for`/`genvar`,
`UnrollGenerate` pass, T25) and connect-module insertion (ch. 7,
`InsertConnectModules` pass, T52) are both derived-artifact producers
under the same rule `FlattenHierarchy` already upholds — neither writes
into `Design::modules`. A `generate` block's authored source (the loop
itself, unexpanded) is what a reader gets back from `Design::modules`;
its unrolled instances land only in the derived flat form
(`Design::flat_modules`, consumed by `FlattenHierarchy`/`CircuitCompiler`
the same way any other flattening input is). A connect-module bridge
`InsertConnectModules` inserts for a genuine cross-discipline mismatch is
recorded in `Elaborator::generated_instances` (the same derived-only
field `UnrollGenerate` writes to) — never spliced into the authored
module's own instance list. `crates/piperine-lang/tests/vams_generate.rs`
(`for_generate_is_absent_from_the_authored_module`) and
`crates/piperine-lang/src/elab/lower/connect.rs`'s own unit test
(`matched_bridge_lands_only_in_generated_instances_not_design_modules`)
are the regression proofs. This is not a new rule — it is MD-25's existing
invariant, restated for the two VAMS-native constructs that could
plausibly tempt a future pass to mutate `Design::modules` directly, the
same way the rule was originally drafted against `FlattenHierarchy`
itself.

### MD-26: Introspection metadata = atomic attributes, not role bundles

PHDL device-introspection metadata is declared with small, single-purpose
attributes that compose on any declaration — `@name`, `@unit`,
`@description`, `@kind` — rather than role-shaped bundles
(`@opvar(...)`/`@observable(...)`/`@terminal(...)`). Model identity is the
one deliberate pair: `@model(type, version)` carries both fields in one
attribute (a model type is meaningless without noting its version; user
2026-07-23). Consequences:

- The opvar-name vs observable-name inconsistency dissolves at the root: a
  `var` carries ONE `@name`, read by both the opvar query catalog and the
  observable catalog — nothing to "unify".
- `@kind` is placement-resolved: on a `var` it names an `ObservableKind`,
  on a port/wire a `TerminalKind`. One attribute, interpreted by what it
  annotates.
- Terminal classification gets its OWN attribute surface — never `@port`
  (plugin device-wiring, plugin-scope only) nor `@rfport` (RF S-param
  ports). Distinct purpose, distinct surface.
- Every attribute is optional (zero regression on existing stdlib models)
  and a textual `extern attribute` declaration (MD-24 — LSP go-to-def).

`$limit`'s limiter naming stays an **operator argument** (thread the
existing `$limit(kind, ...)` `kind` + optional reason into
`LimitingReport`), not an attribute — it is a call-site concern, outside
the metadata-attribute family.

**Status:** Locked (user, 2026-07-23 — `phdl-introspection-attributes`
Specify phase; spec in `.specs/features/phdl-introspection-attributes/`,
PIA-01..20, Design pending). Makes the reflective ABI bridge delivered by
`element-abi-maturity` declarative (author-controlled from PHDL).

### MD-27: Host library — ideal-first, host-pure, api-canonical, parity-enforced

The P3 host library (`import piperine` + the Rust `piperine-api` surface) is
designed **ideal-first**: the perfect surface is drawn greenfield
(`.specs/features/host-library/ideal.md`) and only then diffed against what
ships (`delta.md`). Four locked constraints govern the build:

1. **api-canonical.** `piperine-api` (Rust) is the single source of truth;
   `piperine-python` is a thin wrapper. Every capability lands in the api
   first; Python binding is mechanical after. This kills host drift by
   construction (the exact bug found 2026-07-23: Rust ran `sens`/`pss`/`pz`/
   `sp`/`disto` while Python had no typed result).
2. **MD-22 enforced by a parity test.** Uniformity is not intent — a
   `tests/host_parity.rs` enumerates both public surfaces and fails loud on
   any name/shape/result-type/enum/error divergence. "Rust too, in principle"
   means the Rust mirror is designed *with* Python, never bolted on.
3. **Host-pure scope.** The feature delivers host surface only —
   `Session`-centric analyses, the element-abi introspection door (opvars/
   observables/limiting/param-bounds), rich Waveform, sweeps, configs/units/
   errors. `pip.optimize` (design centering) is **P6**; Python plugin
   scripting is **P5**. The feature makes the optimizer *feedable*
   (`Param.bounds`, opvar objectives) but does not implement the loop.
4. **Return-type taxonomy: separate if ops differ, unify if only data
   differs.** Nine types survive; `Trace<T>` is generic (folds `AcTrace`/
   `NoiseTrace`); `Waveform`/`ComplexWaveform` stay split; structured results
   stay distinct. Reshaped once (Phase 1), never build-then-remove.

**Status:** Locked (user, 2026-07-23 — `host-library` Specify+Design+Tasks;
`.specs/features/host-library/`, HOST-01..28, 30 tasks, Execute pending).
Refines ROADMAP P3.

### MD-28: Test placement standard — unit inline, integration grouped by feature

Every test lives where its scope says it should:

1. **Unit tests live inline with the implementation** — a `#[cfg(test)] mod
   tests` block in the *same* `.rs` as the code under test, never a distant
   file. A unit test that references private items or one function's branches
   belongs next to that function.
2. **Integration tests are grouped by functionality** — one `tests/<feature>.rs`
   per behavior/feature area, not scattered by accident of authoring. Tests
   that exercise a crate's public surface across modules are integration tests
   and belong in `tests/`, named for the feature they prove.
3. **Redundant tests are deleted** — coverage is the metric, not count. Two
   tests asserting the same behavior are one test; delete the weaker.

Governs the P6 "test sanitization" workstream (~800 tests, crate by crate,
suite green throughout) and every future test authored. The `tlc-spec-driven`
Test Coverage Matrix already encodes this per layer; MD-28 is the durable
project-wide rule the matrix instantiates.

**Status:** Locked (user, 2026-07-23). Feeds ROADMAP P6 (Cleanup &
completeness) and applies to all new tests going forward.

### MD-29: Plugin contributions are declaration-coupled (revises MD-21)

MD-21 (locked 2026-07-18) said the lifecycle registry is "exposed to Python
so a plugin self-registers (attribute schemas, hooks, scripts, devices)."
Revised by `plugin-interface-v2`: there is **no imperative attribute-schema
self-registration and no per-plugin `extern.phdl`**. Hooks, scripts, and
devices are still contributed, but only via declaration-coupled decorators
(`#[pip::…]` in Rust, `@pip.…` in Python) and the `@device` attribute —
never a hidden `Registrar` call. The stdlib `@device`/`@port` schemas stay
seeded from `headers/device_port.phdl` (MD-24 unchanged). Backends remain
native dlopen + embedded Python only (MD-21's backend half is untouched).

**Status:** Locked (user, 2026-07-25 — `plugin-interface-v2` Execute start,
spec PLG-01..26, context D1–D14). Records the revision the feature's
context.md ("Supersedes / tensions") prescribes.

### MD-30: A plugin is a contributing dependency — one install, two spellings

`piperine add <git>` is the whole install (D9). A resolved package is a
plugin when it **declares contributions**, in either spelling: a dedicated
`piperine-plugin.toml`, or a `[plugin]` section (with
`[plugin.permissions]`) in its own `Piperine.toml`. `Resolver::resolve_plugins`
therefore returns the `[plugins]` entries **plus** every `[dependencies]`
entry that declares contributions, and a plugin package is an importable
`SourceMap` namespace like any dependency — which is what makes D10 true
(the author writes `@device pub mod …` in the plugin's own PHDL; the user
`use`s it and never writes `@device`). The explicit `[plugins]` section
survives for artifact-only plugins with no PHDL to import, and wins for a
name declared in both. A dependency declaring neither manifest spelling is a
plain PHDL library and contributes nothing (no dependency is a plugin by
accident).

**Status:** Locked (2026-07-25 — `plugin-interface-v2` post-Execute audit;
`validation.md` findings 1–3). Makes the delivered surface match `design.md`
§1 and `ideal.md` D9/D10.

### MD-31: A policy invariant lives in the gate, not in a document

Every project rule that can be checked mechanically is a test in
`cargo test --workspace`, not a paragraph someone is supposed to remember. P6
found two test files switched off with `#![cfg(any())]` — 38 tests, one file
named in `CLAUDE.md` as a test of record — dark for months while reading as
coverage, and two `ElementCapabilities` bits that nothing consumed. Both were
already "documented"; documentation caught neither.

The shape is a **registry + exhaustiveness assert** (the pattern
`capabilities_contract.rs` established): enumerate the real surface from the
tree or the spec file, look each item up in a table that must account for it,
and fail naming what is unaccounted for. The guards this locks in:
`tests/suite_hygiene.rs` (no disabled test code, no `#[ignore]`, every ignored
doc example registered with its reason, every integration target declares its
scope in a `//!` header), `capabilities_contract.rs`
(`no_capability_flag_is_merely_reserved`), and
`spec_failure_rules_guard.rs` (Part VII §16 rows bound to tests or explicitly
marked). A guard must be proven able to fail — inject the violation, watch it
fail, revert — or it is decoration.

Extends **MD-28** with its enforcement: MD-28 says where tests live, MD-31 says
the rule is a test.

**Status:** Locked (2026-07-26 — `p6-cleanup-completeness`, CLN-05/08/13/19).

### MD-32: `BYPASS_OK` is per-circuit consent

The DC stamp-bypass cache applies only when **every** element in the circuit
declares `ElementCapabilities::BYPASS_OK`. A device declares it only when its DC
stamps are a pure function of its terminal voltages — no history-dependent
operator slot (`delay`/`slew`/`transition`/`idt`), no runtime event, no `$limit`
limiter, no diagnostic, no digital half. A `ddt` contribution does not
disqualify: charge is not a DC stamp.

Before P6 the flag was write-only and the cache applied to every circuit whose
solution stopped moving, including devices that never opted in — a stale stamp
can satisfy the convergence test and lock in a wrong operating point. One
non-declaring element now disables the cache for the whole circuit; correctness
outranks the bypass hit-rate. New device authors: declaring the bit is a promise
about your stamps, and `piperine-codegen`'s predicate
(`AnalogKernel::dc_stamps_depend_only_on_terminal_voltages`) is the reference
reading of it.

**Status:** Locked (user, 2026-07-25 — "wire it"; delivered
`p6-cleanup-completeness` CLN-12).

### MD-36: Runtime semantics live in `piperine-signal`, a pure crate

Piperine holds two categories of logic that had been sharing files: **compile-time
machinery** (symbolic differentiation, Cranelift emission, CSE, the `SimCtx` ABI,
POM lowering) and **runtime semantics** (what `transition` means, how pnjlim
converges a junction, what `table` interpolates, how a reduction accumulates).
The first is `piperine-codegen`'s job. The second is not — it lives there only
because codegen was its first consumer, and it is why the crate reads as mystery
rather than machinery.

Runtime semantics move to **`piperine-signal`**, governed by one load-bearing
invariant:

> `piperine-signal` computes values, derivatives, event predicates and running
> reductions. It **never stamps, never sees a matrix, never sees an `Element`,
> and depends on no other crate in the tree.**

That purity is the sorting rule (if it needs a matrix or a compiler, it stays in
codegen), the reuse argument (the same accumulator reduces a stored `Waveform`
and a live transient), and a **mechanical gate** — a test asserts the empty
in-tree dependency list, not a reviewer (MD-31).

Consequences:

- **Extraction, not invention.** The runtime operators
  (`device/analog/operators.rs`), the event detectors (`events.rs`), `table`
  (`resolve/pom/analog_ops.rs`) and the junction limiter (`limits.rs`) are
  *already pure*; they move with their tests intact. Rules like `transition`'s
  re-anchor, the `above`-vs-`cross` seeding asymmetry, and the five pnjlim rules
  were each learned from a failure — rewriting from an operator's *name* would
  silently drop them.
- **Capability parity becomes testable.** A host-written device reaches the same
  vocabulary a PHDL device does, proven by writing the same device both ways and
  asserting identical results. That is the success criterion, not the slogan.
- **Two features, sequenced.** `dv-tester` **extracts** and pays the interim cost
  (per-family cross-check fixtures, both paths asserted equal);
  `codegen-simplification` **delegates**, deleting codegen's copies. Splitting
  them keeps a correctness-critical refactor off the critical path of a feature
  whose value is elsewhere.

**Status:** Locked (user, 2026-07-31 — `dv-tester` design;
`.specs/features/dv-tester/ideal.md` §3, §8.5 and
`.specs/features/codegen-simplification/ideal.md`).

### MD-37: One device concept, two binding modes

There is **one** way to answer "how do I write a device": declare it in PHDL with
`@device`, which already exists (MD-24) as a module that declares its ports and
params and defers its implementation. What varies is only *where the
implementation is bound*:

| Binding mode | Implementation | Bound at | Distribution |
|---|---|---|---|
| **plugin** | a dlopen'd cdylib, `#[pip::device]` + `inventory` | plugin load, TOFU-pinned (MD-30) | a release artifact |
| **host** | a class/impl in the running process | session build | the script itself |

Both converge on `Element` through a blanket adapter, so the solver cannot tell
them apart (MD-01). The PHDL never changes between modes; circuit build resolves
an `@device` module to a plugin binding or a host registration and fails loud
naming both when neither exists.

- **No new macro.** A Rust host device implements `HostElement` and registers
  explicitly; `#[pip::device]`'s `inventory` collection is a plugin-binary
  mechanism with no meaning in-process, and MD-13 rule 5 makes not adding one the
  correct answer rather than a compromise. Python's `@pip.device` performs the
  same registration in that language's idiom.
- **Promotion is the distribution story.** A Rust host device becomes a plugin
  device with **no change to the PHDL** — prototype in-process, promote when it
  stabilizes, and the circuits using it never notice. A Python host device cannot
  graduate (there is no Python cdylib) and stays script-local: fine for testers,
  limiting for models, and stated rather than discovered.

**Status:** Locked (2026-07-31 — user chose PHDL-declared host devices and
delegated the `#[pip::device]` relationship to the agent;
`.specs/features/dv-tester/ideal.md` §8.6–§8.7).

---

### MD-38: ngspice parity is measured, not asserted

A model is "ngspice-faithful" only where a golden cross-check can falsify the
claim. The audit of 2026-07-28 marked noise and temperature ✅ on models the
harness never exercised in those domains — `tests/ngspice_validation.rs` covered
DC operating points, DC sweeps, `.disto`, `.four` and `.pz` only, while the
known-wrong behaviour (saturation-only `gm` in channel noise, absent switch
noise, dead `ic`/UIC) sat precisely in the uncovered domains.

The rule: **extend the oracle before closing the gap.** A parity fix ships with
a golden test that fails when the fix is reverted; a ✅ with no test behind it is
a defect in the table, not a property of the model. Each domain (DC, AC
magnitude, AC phase, noise PSD, transient, temperature) carries an explicit
documented tolerance with a stated rationale — never an undocumented constant in
a test body.

Corollary — matching the model, not the defect: where ngspice carries a known
historical bug, piperine documents the deliberate deviation and asserts its own
value. Bug-for-bug fidelity is not the goal.

**Status:** Locked (2026-08-04 — `.specs/features/ngspice-parity/spec.md`).

### MD-39: No device that PHDL already is

A SPICE device earns a port only when it adds capability PHDL lacks. Two classes
do not:

- **Behavioural sources (`asrc`, the `B` element).** `B` exists because a SPICE
  netlist has no expression language. PHDL does — `V(p,n) <+ <expr>` in an
  `analog` block *is* the behavioural source, symbolically differentiated like
  any other contribution. An imported deck's `B` line becomes a PHDL body via
  the SPICE→PHDL importer, so nothing is blocked. Porting it natively would
  reimplement the language's premise as a model.
- **Devices outside the stated user.** The north star is the low/medium-level
  designer without Cadence-class tooling. Foundry-grade models serving a
  different user (`vbic` for RF BJT) wait for measured demand rather than
  shipping on the assumption that more models is better.

This does not exempt genuinely missing physics: BSIM is in scope (as its own
features), because a compact deep-submicron MOS model is capability PHDL cannot
express as a wrapper over what exists.

**Status:** Locked (2026-08-04 — user rejected `asrc`/`vbic` on merit during
`ngspice-parity` specification). **Superseded in subject by MD-42
(2026-08-13, `verilog-ams-support` T58):** the reasoning above survives
unchanged — a device earns a port only when it adds capability the
frontend lacks — but "PHDL" throughout is now read as "Verilog-AMS", the
frontend PHDL was retired in favor of. `V(p,n) <+ <expr>` inside an
`analog begin...end` block is the VAMS-native behavioural source with the
identical role `B` would otherwise fill.

---

### MD-44: Verilog-AMS is the sole frontend — PHDL dies now, no dual-frontend S2

The `verilog-ams-support` feature spec/design/tasks planned a Phase-5 "S2"
dual-frontend period: both PHDL and VAMS elaborate side-by-side so
differential parity (VAMS-06) can be measured, with the PHDL parser deleted
only at T38 (Phase 7). **Re-spec'd mid-Execute by the user (2026-08-12,
during T21):**

> "piperine-lang morre, o vams passa a ser o parser principal e o POM tem
> que se adaptar PARA representar 100% do VAMS"

Concretely, from T21 onwards:

1. **`piperine-lang::parse::ast` (PHDL AST) is dead.** `BehaviorStmt` and
   every other POM re-export repoint to `piperine_vams::ast::*` directly.
   There is no transitional "both compile" period; the PHDL path goes dark
   the moment T21's swap lands.
2. **The POM adapts to VAMS, not vice-versa.** Where the spec called for
   "VAMS constructs lower into the existing POM node X", the truth is the
   POM node IS the VAMS construct — `Nature`, `Branch`, `discipline`,
   `genvar`, `branch`, `paramset` are first-class POM nodes whose shape is
   the LRM's shape, full stop. No "map VAMS onto PHDL-shaped POM" step.
3. **Test debt accepted**: every PHDL-elaboration test that touches
   behaviors/contributions/events will fail to compile from T21 until
   either the test is rewritten against VAMS source (corpus migration,
   Phase 8) or the test is deleted as part of T38 (PHDL parser removal).
   The `cargo test --workspace` gate is therefore relaxed to "0 rustc
   warnings + every *VAMS-originated* test still passes + every
   PHDL-originated compile failure is documented in tasks.md's progress
   log per task". The pre-T21 baseline (468 passing in `piperine-lang`)
   is the floor VAMS-originated tests must stay above.
4. **T38 (PHDL parser deletion) collapses into T21**: there is no
   separate "delete parse/" task because T21 *is* the deletion. T38's
   surviving deliverable (the `.phdl` monotonic-shrink gate, T39) stays.
5. **Differential parity for VAMS-06 is achieved differently**: the RC +
   counter fixture is authored once in `.vams`, and parity is measured
   against the existing PHDL-fixture *recorded numeric result* (committed
   golden), not against a live PHDL compile. The golden survives PHDL's
   deletion as a static oracle.

This MD supersedes the S2 column of `design.md` §7's stage table and
the dual-frontend clarification paragraph below it. T22–T37 are
re-sequenced in `tasks.md` accordingly: each task that touched "both
frontends" or "PHDL path still compiles" drops that constraint.

**Status:** Locked (user, 2026-08-12 — re-spec of `verilog-ams-support`
Execute phase, supersedes design.md §7 S2 row).

### MD-45: T21's PHDL-AST deletion takes the value-layer language with it — dv-* goes dark until rebuilt on VAMS attributes

T21's Phase-5 batch worker found a gap MD-44/`design.md` didn't account for:
`piperine-lang/src/parse/ast.rs` is not only "PHDL's Verilog-AMS dialect" —
it also carries a full Rust-like value-layer language with no VAMS Annex A
counterpart: generics (`TypeParam`, const/type params), the capability
system (`CapabilityDecl`/`ImplDecl`), `BundleDecl`, `EnumDecl`,
`ConstraintDecl` (dv-core's `require`/`var`/`target`/`cover` blocks),
`FnDecl` with `Pattern`-based `match`, tuples, lambdas, `Map`/`Set`
literals, optional types. `piperine-vams`'s AST (LRM Annex A grammar) has
none of it — there is no production to lower from. Twelve files outside
`pom/behavior.rs` depend on this machinery
(`elab/event.rs`, `elab/lower/{attrs,constraint,mod,mono,register}.rs`,
`elab/registry/{callables,impls,mod,types}.rs`, `elab/typecheck.rs`,
`eval/interp.rs`, `pom/design.rs`), and it backs three DELIVERED features:
`dv-tester`, `dv-gradients`, `host-library`.

**Decision (user, 2026-08-12): gut it now, rebuild later.** T21 deletes
`piperine-lang/src/parse/` in full, including this value-layer machinery —
no minimal parallel AST kept alive. `dv-tester`/`dv-gradients`/
`host-library`'s capability/generics/bundle/enum/constraint-block-dependent
surface goes dark (fails to compile or loses functionality) from T21 until
a later phase rebuilds the needed pieces on VAMS's `(* attribute *)`
surface — the path `design.md`'s P2 story already sketches for
attribute-carried extensions (T35 is the named landing task for
`@device`/`@port`; the rest of the value-layer surface has no task yet and
needs one before Phase 7 closes). This is truer to MD-44's "no dual
anything" than keeping a second AST alive, at the cost of a known
functionality gap in three delivered features until it's closed.

**Consequence for tasks.md:** T21 proceeds today as literally specified
(no scope-up, no parallel AST). A new task must be added — before Phase 7
(T34–T39, downstream retarget) closes — to rebuild the capability/generics/
bundle/enum/constraint-block surface on VAMS attributes, or to explicitly
retire dv-tester/dv-gradients/host-library's dependence on it. Until that
task exists and lands, `dv-tester`/`dv-gradients`/`host-library` are
**regressed, not just paused** — this MD is the record of why and the
open item tracking the debt.

**Status:** Locked (user, 2026-08-12 — resolves the T21 batch-worker
blocker; supersedes the "mechanical retarget" framing in `design.md` F1/F2
for the value-layer subset of `piperine-lang/src/parse/ast.rs`). The
constraint-block open item is **closed for the require core** by MD-49
(2026-08-17); `var`/`target`/`cover`/distributions remain open follow-ups.

---

### MD-46: "Solver out of scope" means no analog-solver/`Element`-ABI redesign — the digital event scheduler is in-scope for VAMS-20

`verilog-ams-support/design.md` §8 flagged R7: `spec.md`'s Out of Scope
table says "changing `piperine-solver`/... is out of scope," but VAMS-20
(full LRM ch. 7–8 mixed-signal scheduling — `InsertConnectModules` +
stratified event regions) plausibly requires changes inside
`piperine-solver`'s digital scheduler. R7 flagged this as a contradiction
that must be resolved by explicit user confirmation before S5 (Phase 9)
proceeds, not silently assumed either way.

**Decision (user, 2026-08-12, T50):** "solver out of scope" in the Out of
Scope table means no redesign of the **analog** solver or the `Element`
ABI. Extending `piperine-solver`'s **digital** event scheduler
(`crates/piperine-solver/src/digital/`) to the LRM ch. 8 stratified
simulation cycle (active/inactive/NBA/monitor region ordering) is
in-scope-by-necessity for VAMS-20 and is not blocked by that row. T51
(stratified event scheduler) and T52 (`InsertConnectModules`) proceed on
this basis; neither may touch `core/element.rs`'s trait surface, MNA/
matrix code, or anything under `analyses/` — that boundary is what stays
frozen.

**Consequence for tasks.md:** T51's `Where` (`kernel/digital/` and/or
`piperine-solver/src/`) is confirmed correct as scoped; no re-scoping or
dropping needed. `spec.md`'s Out of Scope row is amended in place (this
commit) to state the digital/analog split explicitly, rather than leaving
R7's tension unresolved in the reader's head.

**Status:** Locked (user, 2026-08-12 — resolves design.md §8's R7; the
spec amendment itself is `spec.md`'s "Changing `piperine-solver`/..." Out
of Scope row, amended in the same commit as this entry).

---

### MD-42: Verilog-AMS is the frontend; PHDL is retired

`verilog-ams-support` (Phase 1–10, T1–T57) replaced Piperine's own PHDL
language with standard Verilog-AMS (LRM 2.4) as the sole frontend — not a
dual-frontend period, not an opt-in alternative. `piperine-vams` (LRM
Annex A grammar only, C7) parses; `piperine-lang` elaborates the parsed
AST straight into the POM. Zero `.phdl` sources remain in the tree
(`tests/suite_hygiene.rs::phdl_corpus_does_not_grow` is the regression
guard). This decision has two concrete effects on the project's earlier
locked decisions:

1. **Supersedes MD-39's subject, not its reasoning.** MD-39 ("no device
   that PHDL already is") argued that a SPICE device earns a port only
   when it adds capability the frontend lacks. That reasoning is sound
   independent of which frontend it's stated against — only its subject
   changes, from PHDL to Verilog-AMS. See MD-39's own status line for the
   pointer back.
2. **Re-spells MD-24.** "Every referenceable name resolves to a textual
   declaration" was PHDL's `extern type`/`fn`/`task`/`operator`/
   `attribute`/`impl` mechanism — a PHDL-syntax-only construct with no
   Verilog-AMS Annex A production, deleted in full (not retargeted) along
   with `piperine-lang/src/parse/` (MD-45). What survives of MD-24's
   *principle* ("no silent fallback into a Rust-native registry for
   something that should be declared") is narrower now: only
   `@device`/`@port`, re-anchored on LRM Annex D + `(* ... *)` attribute
   instances instead of `extern` declarations. Every other previously-
   `extern` surface (primitive types, math intrinsics, system tasks,
   runtime operators, cast methods) is now a native LRM builtin the
   parser/elaborator/codegen recognize directly — Verilog-A never needed
   a textual declaration for `sin`/`ddt`/`$display` either, and Piperine
   no longer invents one. See MD-24's own status line for the pointer
   back, and `CLAUDE.md`'s "Declared language surface" section for the
   current shape.

**Status:** Locked (user, 2026-08-12, mid-`verilog-ams-support` Execute —
the pivot itself first recorded at MD-44/MD-45; MD-42 is this decision's
permanent home in the numbering, added retroactively at T58 per
design.md §9's plan to record it once the design was approved). Recorded
2026-08-13 (T58, Phase 10 close).

---

### MD-43: `piperine-vams` is a zero-dependency, purity-gated parse crate

`piperine-vams` (the Verilog-AMS lexer/preprocessor/parser, MD-42) has an
empty `[dependencies]` — no `thiserror`, no `miette`, no filesystem I/O.
`` `include`` resolution is injected via a trait `piperine-lang`
implements (`IncludeResolver`), keeping the parse crate pure while
`` `include`` still works, reusing the existing `SourceMap` search-path
logic. Its own errors are a hand-written typed enum carrying every
structured field (including the LRM clause citation, E-STD rule 5)
plus one `Display` impl and `std::error::Error` — no `thiserror`
convenience macro, because the crate cannot depend on it.
`crates/piperine-vams/tests/purity.rs` is the mechanical guard (mirrors
`piperine-signal`'s own purity guard, MD-36).

This is the **second** sanctioned exception to MD-15's no-proliferation
stance (the first being `piperine-ir`, on the same grounds: a genuinely
distinct concern — grammar/lexing purity — that a shared crate cannot
carry without pulling in dependencies the shared crate has no business
holding). A new crate is not "proliferation" when its whole reason to
exist is the isolation MD-15 itself would otherwise ask a single crate to
fake with internal module boundaries.

**Status:** Superseded by MD-47 (2026-08-13) — the purity-gate boundary this
decision locked in was dissolved the same day, on the user's explicit
call ("Certeza que o purity pode ser obsoleto!"). Kept here as the record
of why the split existed in the first place; MD-47 is the current state.

### MD-47: `piperine-vams` merged into `piperine-lang`; the purity gate is retired

User decision, post-Phase-10 close: MD-43's parser/elaborator crate split
was a real architectural cost — an external-crate seam between "text ->
AST" (`piperine-vams`) and "AST -> POM" (`piperine-lang`) that nothing
downstream ever depended on independently; every consumer reached both
crates together, always in that order. The purity gate it protected
(`crates/piperine-vams/tests/purity.rs`, zero-dependency + banned-token +
provenance scan) is dropped outright — not weakened, deleted.

**Mechanics:** `piperine-vams`'s `ast`/`grammar`/`lexer`/`preprocessor`/
`parser`/`error`/`fmt`/`keywords` modules and `piperine-lang`'s `elab`/
`pom`/`eval`/`annex_e`/`value`/`source_map`/`math`/`vams_include` modules
now live in one crate. The physical merge landed inside the (then)
`piperine-vams` directory — its own `[dependencies]` already carried
everything the elaborator needs (`thiserror`, `miette`, `serde`,
`serde_json`, `num-complex`) — and the crate was renamed `piperine-lang`
afterward, so every downstream `piperine_lang::…` path and every
`piperine-lang = { path = "../piperine-lang" }` Cargo dependency line
kept working unchanged; only the five crates that depended on **both**
crates (`piperine-codegen`, `piperine-lang-server`, `piperine-api`,
`piperine-cli`, `piperine-python`) needed their now-redundant
`piperine-vams` dependency line removed and `piperine_vams::` call sites
repointed at `piperine_lang::`.

**Verification (not self-assessed — A/B'd against the pre-merge tree via
`git stash`):** every test target that compiled before the merge compiles
after it; every test that passed before passes after; the two crates'
pre-existing failure sets (`piperine-lang`'s 22 dead-PHDL-AST-debt targets
— `elab.rs`/`parse_elab.rs`/`extern_grammar.rs`/etc — and
`piperine-codegen`'s 6, e.g. `digital_jit`/`param_diff`) are byte-identical
pre- and post-merge, confirmed target-by-target, not just in aggregate
count. `cargo build --workspace`: zero warnings, both before and after.
`suite_hygiene`'s `no_dead_architecture_identifiers` and
`every_integration_target_declares_its_scope` both stay green post-rename.

**Status:** Locked (user decision, 2026-08-13).

### MD-48: `Span` is a resolved, human-facing type — `PreprocessedSpan` is the internal, unresolved one

`preprocessor-span-mapping` design decision (2026-08-16). Today's `Span
{start, end}` was a flat byte offset into the preprocessor's expanded
output buffer, silently misused at every render boundary as if it
indexed the *original file's own text* — the root cause of the
`vams-corpus` sweep's `OutOfBounds` diagnostic crashes. Fix: rename
today's type to `PreprocessedSpan` (pure mechanical rename, no logic
change, ~80 call sites across `grammar/*.rs`/`lexer.rs`/
`piperine-codegen`) and introduce a new `Span {source_id, start, end,
expansion_chain}` that can only be constructed via a new
`SourceProvenance::resolve(PreprocessedSpan) -> Span` registry (design
doc: `.specs/features/preprocessor-span-mapping/design.md`). Convention
for future features: **any position type minted before final
file-identity is known must not share a name with the resolved,
render-ready type** — a bare offset must never be presentable to a
diagnostic/LSP boundary as if it were already real. `SourceProvenance` is
deliberately named to avoid colliding with the existing
`piperine_lang::SourceMap` (namespace → `` `include `` search-path
resolver, `source_map.rs`) — a different, unrelated concept.

**Status:** Locked (implemented and Verifier-PASSed 2026-08-16, 17
commits on `feature/device-verification`, `3847fc18`..`cef91386` — see
`.specs/features/preprocessor-span-mapping/validation.md`). `piperine
expand`/`--trace` (SPANMAP-09/10/11) shipped as part of the same
feature, added mid-flight per user request.

---

### MD-49: `constraint begin … end` is a sanctioned Piperine extension — contextual keywords, module-body block

`constraints-on-vams` design decision (2026-08-17). Piperine's
design-verification surface (declared constraints → signed margins →
postures → findings channel) needed a grammar home after MD-45 deleted
the PHDL value layer. The user's call, superseding MD-45's tentative
"rebuild on `(* attribute *)`" path for this surface: constraints return
as a **`constraint begin … end` block inside the module** — one more
module body mirroring `analog begin … end`, so ports/params/branches are
simply in scope, no name-matching indirection.

Key terms locked with the user (do not relitigate; spec
`.specs/features/constraints-on-vams/`):

- **Grammar:** Annex A has no constraint production, so this is a
  deliberate Piperine extension — the second sanctioned *new production*
  after the `///` doc comments (§1.1). `constraint`/`require`/`tol` are
  **contextual keywords**: identifier text matched only at their
  production sites, never added to Annex B's reserved set (the
  `annex_b_keywords` gate pins `KEYWORDS` to Table B.1 — extending it
  would be silent conformance drift).
- **Severity:** postures map onto LRM §9.7.3's assertion severities
  (`strict`→`$error`, `collect`→`$warning`, `off`→silent), inheriting the
  standard's rejected-iteration rule (violations inside a rejected Newton
  iteration are never reported) instead of re-deriving it.
- **Core scope:** `require <name> : <pred> [tol <scale>];` only.
  `var`/`target`/`cover`/event-block scoping/temporal windows/parameter
  distributions are named deferred follow-ups (the random-variable
  catalogue direction lives in the feature's `context.md`).
- **`tol` placement:** a trailing clause on `require` (its PHDL host
  `target` is deferred, and normalization is what makes margins
  comparable — it moved rather than waited).

Producer: `grammar/item.rs::parse_constraint_block` →
`pom::PomRequire`/`Module::constraints()` → codegen
`ConstraintLowering` → `LoweredBody::constraints` → the preserved
kernel/device/solver/api consumers. Reconnect-not-rewrite: nothing below
the codegen `resolve` stage changed.

**Status:** Locked (user, 2026-08-17; producer landed same day — see
`.specs/features/constraints-on-vams/`). MD-45's open item "rebuild the
constraint-block surface" is hereby **closed** for the require core; its
remaining deferred statements each need their own feature before
dv-tester/dv-gradients are fully lit again.

---

### MD-51: conformance is declared at the test; the gate derives the matrix

`lrm-conformance-wave2` design decision (2026-08-20, Phase 0 "Falsifiability
spine"). Wave 1's `lrm_conformance_matrix.rs` tracked *wave-1 finding IDs*
against a hand-maintained registry — accurate for the 54 findings it closed,
but no mechanical answer to "does Piperine implement the LRM?" existed: the
LRM's 612 headings (486 numbered + 126 annex, `VAMS-LRM-2-4.md`'s own
`^#{2,6} **<id>` structure) had no per-section coverage record at all.

The chosen shape (design Approach C, T4-T7): a test target declares what it
proves with a `//! LRM-COVERS: <ids>` doc-comment header (comma-separated
bare section ids, no `§`, no ranges — `suite_hygiene.rs`'s
`lrm_covers_declarations_are_well_formed` enforces the syntax) — on either
an integration target's file header or an inline `#[cfg(test)]` module
header for the "pure helpers" test layer. `tests/lrm_section_coverage.rs`
is the **derivation**, not a second source of truth: `lrm_sections()`
parses the LRM's own headings into the 612-id ground truth,
`declared_coverage()` scrapes every `LRM-COVERS:` line in the tree, and
`covered_and_excluded_partition_the_lrm` asserts every id is either
declared-covered or excluded (`coverage_exclusions.toml`'s
`Informative`/`OutOfScope` rows, gated to only the LRM's own front-matter
markings or a real spec.md Out of Scope row — never a free pass). No
registry to keep in sync by hand; a fix that adds a real test and declares
its clause closes its own matrix cell in the same commit, and a stale
claim (wrong id, or a file with no live test backing it) fails loud instead
of silently drifting, the same falsifiability bar MD-31 set for
wave-1's registry-style gates.

**Status:** Locked (user, 2026-08-20).

---

## Handoff Snapshot

**Last updated:** 2026-08-06 — `ngspice-parity` DELIVERED (T1–T24 + Fix A/B,
Verifier PASS, 22/22 ACs spec-anchored); `codegen-simplification` DELIVERED
2026-08-03 (T1–T14, Verifier PASS with 2 minor non-blocking gaps); `dv-tester`
DELIVERED 2026-08-01; `dv-gradients` DELIVERED 2026-07-31.

### Feature — `ngspice-parity` (DELIVERED 2026-08-06)

Spec/design/tasks/validation in `.specs/features/ngspice-parity/`. Executes
MD-38 (parity measured, not asserted): extended the golden oracle to AC,
noise, transient and temperature, then closed the deviations it measures
across the 13 shipped models.

**All 24 tasks delivered across 5 phase-batch sub-agents + 1 Verifier round.**
The headline deliverable — `tests/ngspice_parity_matrix.rs`, the MD-31
coverage gate — is the only mechanical way to make "no unfalsifiable ✅"
true: 13 models × 6 domains, every cell `Covered { fixture }` (with a
golden target that actually runs it) or `NotApplicable { reason }`, all
four asserts proven able to fail. `tests/support/ngspice.rs` owns the
shared `NgspiceHarness`, the N-column `WrData` parser, and the
seven-domain `ToleranceContract` (NGP-07: the only place a numeric
tolerance may be written).

**Model fixes (NGP-08..15):** analytic `gm` in MOSFET/JFET channel noise
(T11/T12, killed the saturation-only `gm_est`); switch ON-state thermal
noise (T13); flicker `cox=0` substitution (T14); BJT `VAF`/`VAR` temp
dependence (T15); `ic`/UIC enforced on cap/diode/bjt/jfet/mos1/2/3
(T16/T17); `CKTsrcFact` falsifiable fixtures (T18 — both sources already
honoured via two routes, deliverable was the proof).

**`urc` faithfulness (NGP-16..19, 22):** geometric K taper (T19);
ISPERL diode-mode shunt path (T20); generic `urc[N]` retiring the
hand-unrolled triple (T22) once gap-3's codegen side landed; `FMAX → N`
auto-select via `URC_DEFAULT_N` (T23).

**gap-3 (NGP-20, 21):** `FlattenHierarchy::remap` expands const-indexed
`NetRef` to its scalar net (T21, `8a1c08c`); completed at the codegen
layer by Fix A (`ba79501` — array-wire element registration in
`structure.rs`) and Fix B (`8a8114b` — structural-connection unification
in `builder.rs`). 7 inline flattener tests + 4 inline codegen tests
cover the happy paths and four loud-fail paths.

**Two architectural blockers found and closed during Execute (both
outside the original task scope, both small and surgical):**

1. **UIC default-ic always-clamp.** The `@ initial` mechanism existed
   and worked for explicit `.ic` (SC-12 proves it), but
   `flatten/analog.rs:891 collect_initial_conditions` collected seeds
   unconditionally — wiring `@ initial { V(p,n) <- ic; }` globally into
   `spice::passives::cap` with `param ic : Real = 0.0` default regressed
   every transient where a cap's natural t=0 ≠ 0. Closed by a per-seed
   param-dependency mask in `AnalogInstance::initial_conditions`
   (`instance.rs:679-704`, plus kernel/compile.rs plumbing) that
   suppresses a seed whose backing params are all at declared defaults.
   A second piece: the UIC one-step hold (`transient.rs:748`) was
   released right after the DC OP rather than after the first accepted
   step — otherwise ~step/τ of integration time was "lost" at t=0,
   blowing `uic_rc`'s tight TRAN tolerance. ngspice DC/tran baselines
   bit-identical throughout.
2. **gap-3 codegen integration.** T21 expanded the flattener but left
   the codegen layer unaware of array wires: `structure.rs:99-102`
   registered a lifted array wire as one scalar node (collapsed every
   `tap[i]` onto a single net), and `builder.rs:189-217` never
   processed `top_module.connections()` (silently dropped `tap[0] = p`
   aliases). The existing gap-3 test verified only the flatten shape;
   `run_examples.rs` only elaborated `language_features.phdl`'s
   `RcChain[N]`, never compiled+ran it. Closed by Fix A + Fix B (above).

**Two documented deviations remain (both with reasons recorded in the
model header, ROADMAP, and spec traceability — the three sites the
Verifier audits):**

- **`ind` (NGP-10 part).** ngspice's `ind.IC` is a **current** seed
  (`indload.c:39-42` flux form `L·I`); `collect_initial_conditions`
  only collects `Potential` seeds. A `Flow`-seed mechanism is a
  separate codegen feature.
- **mos/bjt/jfet external-node vs device-internal-state seed (NGP-10
  narrower).** Piperine's `@ initial` pins the external port, ngspice
  seeds the device-internal node and solves series resistances against
  it. The two agree when RD/RS=0 (`uic_mos`'s case) and diverge only
  when RD/RS are nonzero. A device-internal-state-seed mechanism would
  close the divergence.

**Verifier: PASS** — 22/22 ACs spec-anchored with `file:line` evidence
and spec-matching assertions; 4/4 discrimination-sensor mutations killed
(UIC default-ic filter, UIC hold release, urc K-taper, array-wire
element registration); gate `cargo test --workspace` 1841 passed / 0
failed / 0 rustc warnings / 4 pre-existing ignores, +15 tests vs the
T10 baseline of 1826. Three docs-drift gaps flagged in round 1 (ROADMAP
urc row stale; `ind` deviation reason cited the closed JIT-blocker;
mos/jfet external-node deviation imprecise vs bjt) closed in a single
follow-up docs commit. Lesson L-024 distilled: when a Tn task ships a
docs rewrite and later commits close items the docs described as open,
the docs MUST be re-synced in the same commit or the next docs commit.

**Found and fixed alongside, outside the feature's scope:**

- `cranelift_jit` leaks its code pages on drop by design ("leak memory to
  guarantee validity of function pointers"); nothing called the unsafe
  `free_memory`, so every compiled kernel cost the process ~8 kB permanently.
  `emit::OwnedJit` now makes that call — safe because a kernel's `fn` pointers
  live in the same value as its `JITModule` and instances hold it behind an
  `Arc`. Guard: `crates/piperine-codegen/tests/jit_memory.rs`.
- The ngspice scratch-dir key was per (circuit, domain, pid), so two tests
  running one fixture in one domain deleted each other's working directory
  mid-run — a flake that reported as ngspice failing to spawn. The key is now
  per run.
- `[profile.dev] debug = "line-tables-only"`: full debuginfo across ~50 test
  binaries made `rust-lld` the dominant cost (~1.1 GB per link, several
  parallel). Same one-crate relink went 3m47s → 33s.

### Feature — `codegen-simplification` (DELIVERED 2026-08-03)

Spec/design/tasks/validation in `.specs/features/codegen-simplification/`.
Executes MD-36's delegation half: deletes `piperine-codegen`'s second copies
of the five runtime-semantic families `piperine-signal` now owns (runtime
operators, event detectors, the junction limiter formula + bookkeeping,
`table`, and — newly extracted, not just delegated — digital edge
detection), and mechanically guards against a copy reappearing
(`tests/delegation_guard.rs`, 5 witnesses, each proven able to fail per
MD-31).

14 tasks over 7 phases, delivered via 2 sequential phase-batch sub-agents
(T1–T6, T7–T14) + an independent Verifier. One design snag mid-batch-1:
`signal::limit::Limiter::update(vnew, kinds)` recomputes the pnjlim/fetlim/
limvds formula itself, but codegen already gets `vlim` from the JIT (T4/T5
register the same formulas as linked Cranelift symbols) — feeding it through
`Limiter` again would double-compute, and codegen's `vold` lives in the
shared analog state bank (checkpointed generically), not in a struct field.
User-approved fix (SPEC_DEVIATION, T6 commit `b4f2c4e`): a new
`Limiter::advance_precomputed(vlim, vnew, vold: &mut [f64])` method applies
just rules 2 (seed-preservation skip) + 4 (convergence-veto-with-tolerance)
to an externally-computed `vlim` and externally-owned `vold`, sharing its
loop body with the existing `update` via a private helper — no kernel.rs or
instance.rs change, no architecture move.

The one **intended** behavior change (spec's AC 32, found during Specify by
a full 57-file codegen sweep, 2026-08-03): digital edge detection had two
copies that **already disagreed** — codegen's `device/digital.rs` correctly
implements IEEE 1364 (`X→1`/`Z→1` count as rising edges), `piperine-api`'s
host `EdgeWait` only fired on `Zero→One`, silently stalling a host tester
waiting on a clock's first edge out of `X`. New `piperine_signal::logic`
module (`Logic`/`EdgeDirection::fires`, T9) is the one implementation both
sides now delegate to (T10 codegen, T11 host — `EdgeDirection::matches`
deleted, existing narrow-rule test rewritten to the IEEE rule per spec,
never silently dropped).

P2 shrinkage (CGS-12) came in short and was **renegotiated in the open**
per spec's own AC 28 clause, not silently gamed: 3 of 5 capped files missed
their cap (`events.rs` 160/≤140, `limits.rs` 153/≤130, `analog_ops.rs`
375/≤330 — the last now bigger than its own pre-delegation size), and the
crate-wide net-deletion floor landed at 196 of a required ≥300 lines
removed. Full reasoning in `shrinkage.md`: every miss is real adapter/
mapping code the delegation itself required (a `CompiledTrigger`→
`signal::events::Trigger` mapping, T6's `advance_precomputed` plumbing,
`Table`'s `err_from` mapping plus rationale doc comments in a file that
holds 8 operators, `table` being only one) — not code that could be moved
elsewhere to hit the number. User accepted the shortfall as the honest cost
rather than requesting a further cleanup pass.

**Verifier:** PASS — 31/34 ACs matched the spec-defined outcome cleanly, 3
flagged (AC 11's CSE test proves numeric correctness but not the call
count; AC 18's table low-end clamp is untested — `host_capability_parity.rs`
only exercises the high end; AC 26/27 are the pre-accepted P2 shortfall,
independently re-measured and confirmed exact against `shrinkage.md`).
Discrimination sensor: 5 mutations injected in the real tree (no worktree
headroom) and reverted immediately, 3 killed / 2 survived — both survivors
are test-discrimination gaps in otherwise-independently-confirmed-correct
code, not defects, and are filed as follow-up (not blocking). `diff.rs` is
byte-identical across the whole diff range (`resolve/diff.rs` untouched, as
every AC touching differentiation required). `cargo test --workspace`:
1689 passed, 0 failed, 276 test-result blocks, 0 rustc/clippy warnings;
`ngspice_validation.rs` bit-identical throughout (33/33); the four
`cross_check_*` fixtures deleted (their two-copies-agree purpose is now
moot), `host_capability_parity.rs` and `piperine-signal/tests/purity.rs`
untouched and green. Two candidate lessons (L-022/L-023) recorded on the
test-discrimination-gap pattern found by the sensor.

### Feature — `dv-tester` (DELIVERED 2026-08-01)

`ideal.md` / `spec.md` / `design.md` / `tasks.md` in
`.specs/features/dv-tester/`. Scope grew from "a programmatic tester" to **a
host-language device library** — Python or Rust, same contract — of which a
tester is one use. Two rejected drafts are recorded in `ideal.md`'s header; the
second failed on a question as ordinary as *"a PWM comparator reading two
terminals"*, which had no home because its levels conflated abstraction with
rate. The corrected axis is **where the code runs**: level 0 = raw solver ABI
(host call per Newton iteration), level 1 = `piperine-signal` (zero host calls,
MD-36), level 2 = the host library (one host call per test breakpoint) — and the
three **compose in one device**.

A codegen audit replaced two guesses with facts. **`SUPPORTS_ROLLBACK` is live**
(`element-abi-maturity` implemented what `solver-simplification` had removed as a
phantom doc): checkpoint before every attempt, restore on both rejection paths.
**But crossings are not exact and PHDL's own `@cross` is not either** — the
detector compares the *previous accepted* trigger value, `next_breakpoints`
excludes crossings outright, and `EventKind::Crossing` is `priority: Advisory`.
So `@cross` fires at the first accepted point after the crossing, late by up to a
timestep, unreported. Three fidelity tiers are now named; level 1 ships **tier 2**
(predict + `bound_step`, error published) with **no solver change**, and tier 3
(promote `Crossing` to `Exact` + retry) is a scoped solver feature on scaffolding
that already has the right shape.

Decisions taken (full log, `ideal.md` §12): settled-events-only observation
because a mid-delta-cycle value can be a **DAG-evaluation-order artifact** rather
than a signal the circuit had; host devices declared in PHDL (MD-37); digital
edge waits in V1 (a tester that cannot synchronize to a clock is not a tester);
`install` is install-relative with discontinuities **registered, never smoothed**;
`Measure` publishes an **error bound** and `expect` compares against the bound,
with `refine=True` buying exactness; reading an unfed measure is loud. `Measure`
stays a **separate engine** from `dv-core`'s `constraint` (deliberate, cost named
— two definitions of "peak", mitigated by one written semantics + a cross-check
fixture + a shared findings channel), revisited by `codegen-simplification`.

37 tasks over 7 phases; Phase 6 (`cover`) is the declared split point.

**Delivered** via 6 sequential phase-batch sub-agents (T1–T6, T7–T13, T14–T18,
T19–T26, T27–T34, T35–T37) + an independent Verifier, 43 commits
(`ae65570..HEAD` before fixes). New crate `piperine-signal` (pure, zero
in-tree deps, mechanically guarded — MD-36's extraction half) ships `ops`
(delay/slew/transition/idt), `events` (cross/above/timer + tier-2 crossing
prediction), `Behavior`+`BehaviorKind` (closed-form value+Jacobian,
`verify_derivatives` FD oracle), `Measure`+`MeasureKind` (O(1) streaming
accumulators, published error bounds), `Wave`, and the junction limiter —
all extracted from codegen's existing pure copies with semantics intact,
codegen's copies untouched (delegation is `codegen-simplification`'s job).
`piperine-api/src/device/` adds `HostElement` + a `HostAdapter<T>` newtype
blanket-`Element` adapter (Rust orphan-rule workaround, SPEC_DEVIATION from
ideal.md §6's direct-impl sketch), `StampSink`/`Ctx`/`StateView`, and
`@device`'s host binding mode (one PHDL declaration, two binding sites —
plugin or host, MD-37). `piperine-api/src/sequencer/` adds the coroutine
driver (resume strictly after acceptance, D11), `install` semantics, drive
with declared impedance, `expect`/`warn` via `dv-core`'s findings channel,
and the one solver-touching change — `Element::on_digital_settled`, a
broadcast dispatch (not literally a registration list per design.md's
original sketch, but mechanically proven to add no new `EventKind`
variant) consulted strictly after digital net settling. `piperine-python`
bridges via `pip.HostBehavior`/`Wave`/`Measure`/`Sequencer` (closed catalog
in Python, open trait in Rust, §3.7); `tests/host_parity.rs` +
`host_capability_parity.rs` prove name parity and PHDL/host value identity
for four twinned devices. `cover` lands as a `constraint`-block statement
(grammar → elaboration bin-count cap → codegen bin-mapping with its own
`cover=on|off` posture, separate from `checks=` per D9 → host-side merge
across runs naming empty bins).

**Verifier:** FAIL on round 1 (40/45 ACs matched spec outcome cleanly, 4
spec-precision gaps, 1 hard gap; sensor 3/5 killed, 2 survived) — ranked
gaps: DVT-30 (no fixture proved all three levels compose in one class), a
settled-only-dispatch mutant survived (the fixture's glitch net started and
ended at the same value, so pre/post-settle were indistinguishable to the
assertion), a `Wave::pending_edges` right-boundary (`t == end`) mutant
survived, plus DVT-14/16/26/35 spec-precision gaps. Fix round 1 (5 commits)
closed all of them — DVT-30 needed no driver change, only a fixture proving
one `HostElement` stamping directly + installing a `Wave` + owning a
`Sequencer` all at once; both mutants were re-run against the strengthened
fixtures and confirmed killed. Re-verification round 2 found one new Minor
gap: DVT-26's fix correctly proved `WaitCrossing` resumes via tier-2
polling (`accept_timestep`/`StateView`), never through
`EventKind::Crossing` — but `docs/spec/part_viii_host_api.md` and
`ideal.md` §4.1 still claimed the one-to-one `EventKind` mapping. Fix round
2 (docs-only, `6c7b547`) corrected both tables and reworded spec.md's
Sequencer AC #3. **Final: PASS**, 3 lessons distilled (L-018..L-021,
candidates) on settled-only fixture discrimination, window-boundary test
coverage, N-way composition claims needing an actual N-way test, and specs
staying honest when a fix proves a claimed mapping false. `cargo test
--workspace`: 1675 tests across 279 binaries, 0 failed, 0 rustc/clippy
warnings, doc gate clean.

### Feature — `codegen-simplification` (SKETCHED 2026-07-31)

`.specs/features/codegen-simplification/ideal.md`. The follow-on that
**delegates** codegen to `piperine-signal` and deletes its copies (MD-36).
Distinct from `codegen-architecture` (delivered, MD-23), which reshaped the
module tree — this one moves a category of content out of the crate entirely.
Codegen is 16 144 lines with four god-files (`compile.rs` 1 694,
`instance.rs` 1 416, `flatten/analog.rs` 1 329, `kernel.rs` 982); the tree is
already right, so the remaining illegibility is *what is inside*. Hard goal:
**capability parity**, proven by writing the same device in PHDL and in the host
language and asserting identical results. Explicitly does **not** buy
performance. Next priority after `dv-tester` (user).

### Feature — `dv-gradients` (DELIVERED 2026-07-31)

Spec/design/tasks/validation in `.specs/features/dv-gradients/`. Vision:
`.specs/features/design-verification/ideal.md` §3–§5. The analytic sensitivity
+ host policy driver: `resolve/diff.rs` differentiates by parameter (`∂F/∂p`,
additive to the existing Jacobian pass) → codegen emits it as **one**
parameter-indexed JIT function (a `Mos1`-sized ~60-param model card compiles;
this is `.disto`'s function-count wall avoided by construction, §C1) →
`AnalogDevice::param_stamps` + `HAS_SENSITIVITY` is the whole ABI delta (stamp-
perturbation fallback for elements without the bit, always reported by label)
→ a DC adjoint driver (`piperine-solver/src/analyses/`) solves `Jᵀλ = ∂f/∂x`
**once** for all requested parameters (20 params ⇒ 1 extra solve, counted) —
explicitly a **sibling** of the noise adjoint (§12), not a reuse; DC-first
(D6) because `.sens` central-FD is the only verifiable oracle, AC-metric
gradients fail loud naming the gap. Host surface: `sensitivity`/`gradient`
with engine attribution on both hosts (`host_parity.rs`); `SamplePlan`
(`piperine-api/src/sampling.rs`) walks the **authored** instance tree (never
`flat_modules`, MD-25) for Monte Carlo — `global` params share one draw,
mismatch params draw independently, `(seed, index)` fully reproducible and
replayable; `monte_carlo()` reports yield/σ_i/worst-sample, 10³ samples cost
one compile (MD-18); one policy-parameterized `Driver`
(`piperine-api/src/driver/`) — `optimize` (gradient + Nelder-Mead black-box
fallback, engine-attributed), `center` (`min_i(m_i/σ_i)`, σ-scheme stated),
`high_sigma` (mean-shift importance sampling, always reports its estimator +
CI) — forced to `collect` posture regardless of session default; aging as
parameter drift (`piperine-api`) reports the margin delta, refuses loud on a
rebuild-class drift target, and documents honestly that the gap versus
MOSRA/RelXpert is model *data*, not mechanism. `docs/spec/part_vii_solver.md`
+ `part_viii_host_api.md` + `appendix_c_host_surface.md` updated.

Delivered via 4 sequential phase-batch sub-agents (T1–T6, T7–T12, T13–T19,
T20–T23) + an independent Verifier. **Verifier: PASS** — 23/23 ACs
spec-anchored (counted solve numbers, exact draw equality/inequality, exact
compile-count deltas — not just "an assertion exists"), 3/3 discrimination
mutations killed (adjoint solve-count economy, sampling global/mismatch
sharing, driver's forced `collect` posture), one minor non-blocking note (the
`optimize` + `tol`-carrying-param edge case is plausible by construction but
not independently traced to a named test). Two SPEC_DEVIATIONs distilled into
lessons L-016/L-017 (candidates): a policy dispatcher shipping a sibling
policy's real implementation early because Rust needs the whole module tree
to compile as a unit; high-sigma importance sampling scoped to
Gaussian-with-resolvable-σ entries only, documented rather than silently
approximated. `cargo test --workspace`: 1439 passed, 0 failed, 4 ignored,
0 rustc warnings, clippy clean, doc gate clean.

**Previously:** 2026-07-26 — `p6-cleanup-completeness` hygiene subset
DELIVERED (T1–T25). `cargo test --workspace`: 1161 passed, 0 failed, 4 ignored
(illustrative doctests), 0 rustc warnings.

### Feature — `p6-cleanup-completeness` (DELIVERED 2026-07-26)

Spec/design/tasks/audit/validation in
`.specs/features/p6-cleanup-completeness/`. ROADMAP P6's hygiene subset, done
measure-first: `tools/audit_tests.py` inventories every `#[test]` in the tree
with the evidence that classifies it, `audit_verdicts.tsv` records a verdict per
test, and `--check` enforces those verdicts per crate (it is the gate, not the
heuristic — a solver test reaching the crate through `abi` is cross-module
public-surface work, and an inline case building a fixture for its own module's
subject is a unit test of that module).

Landed: **two never-compiled suites** (`ppr_ir.rs` 27 tests, `analog_jit.rs` 11 —
the latter listed in `CLAUDE.md` as a test of record) triaged into 20 restored
tests against today's API plus 18 deletions with survivors named; the
`lang-server` 1880-line `integration_test.rs` split into nine feature suites with
a shared `tests/common/`; the plugin manifest suite moved inline; `phase3.rs`
split into staging/hooks/scripts; four targets renamed off dead vocabulary; the
example-elaboration gate reduced from **three** same-layer copies to one;
`SUPPORTS_QUERIES` removed; `BYPASS_OK` wired (MD-32); Part VII §16 given an
enforcement column with four new rule tests; and three guards added (MD-31).

Corrections to the roadmap's own claims: 1123 tests not ~800; zero ignored tests
(the "28" were dead code); `bound_step_hint` was never dead; §16 has 16 rows not
18. Residue left post-V1: the eight §16 rules marked *not yet enforced*, and the
language/interpreter completeness items.

**Previously:** 2026-07-25 — `plugin-interface-v2` DELIVERED (T1–T17) +
post-Execute audit (four gaps closed, see its `validation.md`).
`cargo test --workspace`: 1123 passed, 0 failed, 0 rustc warnings.

### Feature — `plugin-interface-v2` (DELIVERED 2026-07-25)

Spec/design/tasks/validation in `.specs/features/plugin-interface-v2/`.
ROADMAP P5: the plugin surface collapses to **native dlopen + embedded
Python** (WASM/process backends and `piperine-plugin-wasm` deleted), three
shapes inferred from the manifest keys (pure-PHDL / scripted / device), and
every contribution declaration-coupled through the new
`piperine-plugin-macros` crate (`#[pip::device]`/`#[pip::script]`/
`#[pip::hook(phase)]`, `inventory`-collected per plugin binary) with
name-identical Python decorators (`@pip.…`) locked by
`tests/plugin_parity.rs`. The imperative `Registrar`, the plugin-schema
surface, and the per-plugin `extern.phdl` stub loader are gone (MD-29);
`@device`/`@port` from `headers/device_port.phdl` are the only plugin-facing
schemas. Device binaries distribute as GitHub release assets matched by
target triple, content-hash TOFU-pinned in `Piperine.lock`, with an optional
up-front `verify` and a loud `NoAssetForTriple`; `piperine add` gates on
explicit permissions consent. The five lifecycle hooks are frozen and
`transform_design` staging is the sole device-injection point. The audit
pass added the D9/D10 wiring the task list had missed (MD-30).

**Previously:** 2026-07-23 — `phdl-introspection-attributes` DELIVERED
(T1–T8); independent Verifier PASS. `cargo test --workspace`: 849 passed,
0 failed, 5 ignored, 0 rustc warnings.

### Feature — `phdl-introspection-attributes` (DELIVERED 2026-07-23)

Spec/design/tasks/validation in `.specs/features/phdl-introspection-attributes/`.
Makes the reflective ABI bridge (delivered by `element-abi-maturity`)
**declarative**: device authors control the introspection catalogs from PHDL
with small composable atomic attributes (`@model`/`@name`/`@unit`/
`@description`/`@kind`, MD-26) instead of codegen-derived defaults. The five
schemas ship as a textual `extern attribute` prelude header
(`crates/piperine-lang/headers/introspection.phdl`, MD-24 — LSP go-to-def
inherited). `Design::introspection_meta` resolves them into an
`IntrospectionMeta` sidecar (validated strings, not solver enums — lang's
library is solver-independent), threaded through `CircuitCompiler` →
`PiperineDevice`; each `Introspect` bridge method prefers the sidecar and
falls back to the derived default when absent (zero regression). The
opvar-name vs observable-name inconsistency dissolves at the source — one
`@name` per var feeds both catalogs via a shared `var_display_name` helper
+ a new `AnalogKernel::var_names()` catalog. `$limit`'s limiter naming
(PIA-15..18) is an operator-arg concern: a per-slot
`(name, LimitReason)` catalog on `AnalogKernel` (collected at compile from
each call-site `kind`, reason inferred — `limvds`→VdsStep, else VoltageStep)
+ per-slot active tracking in the device `Limiter` let `limiting_report()`
name the limiter that actually fired (no more hardcoded `"pnjlim"`).
Attribute grammar is keyed-only, so single-field schemas use `value: String`
(authors write `@name(value = "i_d")`) — recorded SPEC_DEVIATION, user-approved.

**Verifier:** independent sub-agent PASS — 19/20 ACs spec-anchored (PIA-16's
optional-reason-arg half is a design-approved MVP deferral — no stdlib model
needs a non-default reason today), 3/3 discrimination mutations killed, gate
849/0. One Minor spec Edge Case gap logged (lesson L-012): `@unit`/
`@description` on a shadowed/non-opvar var is silently accepted by the lang
resolver (shadowing is a codegen concept, unknowable at elaboration) and
dropped by codegen — a codegen-boundary orphan check is the tracked remedy,
not a correctness bug.

### Feature — `hierarchy-flattening` (DELIVERED 2026-07-22)

Spec/design/tasks in `.specs/features/hierarchy-flattening/`. A new
`FlattenHierarchy` elaboration pass (last in `PASSES`, after `Typecheck`)
inlines a mid-level module's sub-instances, wires, and connections into
the parent's flat netlist, recursively, so codegen only ever sees leaf
devices — `device/circuit.rs:389`'s "nested hierarchy" error is now
unreachable for well-formed input. The pass is **non-destructive**
(MD-25): it writes only `Design::flat_modules` (a `#[serde(skip)]`
side map, consumed only by codegen via `design.flat_module(root)`); the
authored hierarchy in `Design::modules` is never mutated, preserving
POM navigability for tools, hosts, and the LSP. The net-rename map binds
child ports to parent nets and lifts child wires to path-prefixed
`inst.wirename` collision-free labels; nesting composes (`u1.s0.r1`).
Cycle detection, dangling-net detection, and indexed-`NetRef` (array-net,
gap-3 deferred) all fail loud. `with_overrides_applied` retargets to
`flat_modules[root]` so the flat-label host contract survives the
inlining depth.

End-to-end proof: the ngspice URC lumped RC line ships as pure-structural
fixed-N modules (`urc2`/`urc5`/`urc10` over a reusable `urc_seg`
submodule in `headers/spice/urc.phdl`). Each module exercises the same
3-level inlining a parametric `urc[N]` would (Top → urcN → urc_seg →
res/cap); the fixed-N route stays inside the MVP flatten boundary (no
array nets, no const-arg-into-behavior). ngspice cross-checked at lump
2/5/10 against an equivalent discrete-R/C ladder (`tests/ngspice/urc_*`,
`tests/ngspice_validation.rs` — DC operating point, each lump value
yields a distinct Vout so the test is discriminating). Monomorph and
restamp regression guards in `tests/urc_compile_count.rs`: every urcN
shape compiles the same leaf-kernel count, and a 20-point `.r` sweep
JITs exactly one build (MD-18), not one per point.

**Authoring note (deferred routes).** The natural `mod urc[N]` +
`StructuralFor` over N segments using an array wire `tap : Electrical[N+1]`
is BLOCKED behind gap-3 — `tap[i]` becomes an indexed `NetRef` and the
flatten pass fails loud (array-net → flat-net expansion is deferred). The
fixed-N modules ship the practical capability now; a future gap-3 follow-
up (per-shape monomorph kernel distinction via `urc__N`) is tracked in
the spec's "Out of Scope" table.

**Next for this feature:** none — delivered. The deferred gaps (2:
const-arg substitution into a monomorph's analog body; 3: array-net
expansion in flatten) are individually tracked and fail-loud today, not
blocking any in-tree device.

### Feature — `declared-language-surface` (DELIVERED 2026-07-21)

Spec/design/tasks in `.specs/features/declared-language-surface/`. Every
referenceable PHDL name now resolves to a textual declaration in stdlib
headers or a plugin's published `extern.phdl` stub — name lookup that finds
no declaration is a fail-loud compile error, never a silent fallback into a
Rust-native registry (MD-24). The seven `extern` forms (`type`/`fn`/`task`/
`operator`/`attribute`/`impl`) cover every previously-magic surface: the 7
primitive value types (`headers/types.phdl`), libm intrinsics
(`headers/math.phdl`), system tasks (`headers/tasks.phdl`), runtime
operators (`headers/operators.phdl`), `@device`/`@port` schemas
(`headers/device_port.phdl`, parsed by `PluginHost::seed_schemas`), and
plugin-contributed schemas (auto-imported `extern.phdl` stub, fail-loud
`PluginError::MissingExternStub` when a schema-contributing plugin
publishes none). Bare-name casts deleted entirely (T17) — replaced by
overloaded `extern impl TypeName { fn from(...) -> TypeName; }` associated
functions, resolved by argument type via T9's new overload resolution
mechanism.

Permanent regression guard at
`crates/piperine-lang/tests/extern_coverage_guard.rs` (T27) — iterates
every native table and asserts a matching `extern` declaration exists, so
the mechanism can't silently regress back into "magic" after a future
change. Discrimination sensor verified by hand: deleting `extern fn cos`
from `headers/math.phdl` produces the named failure.

**Next for this feature:** none — delivered. If the broader ecosystem
(third-party plugins beyond the stdlib's own) ever needs migration, the
mechanism is in place; author an `extern.phdl` stub alongside each plugin
and the existing auto-import path wires it up.

### Feature — `spectral-analyses` (DELIVERED 2026-07-19)

Spec/design/tasks in `.specs/features/spectral-analyses/`. `.four` (Rust
direct-DFT + Python numpy, `Waveform::fourier`), `.pz` (poles via QZ on
`(G,C)`, zeros via the Rosenbrock bordered pencil), `.sp` (per-port
Thévenin excitation + power-wave S-matrix, `@rfport` attribute — no new
device kind), `.disto` (full Volterra HD2/HD3/IM2/IM3 from symbolic
`disto2`/`disto3` JIT kernels) — all four on both hosts (MD-22). ngspice
cross-checked for `.four`/`.pz`/`.disto` (`tests/ngspice_validation.rs`);
`.sp` has no ngspice reference (documented Out of Scope).

**T15's gate surfaced and fixed a real regression** (found by running the
existing ngspice suite, not part of the original task list — logged here
because it changed shared, non-feature-scoped files):
`compile_disto2`/`compile_disto3` (DISTO-01..06) unrolled every ordered
controlling-branch combination into **one** Cranelift function per device.
For a many-branch device (a MOSFET: several controlling terminals) this
never terminated compiling — Cranelift's own codegen doesn't scale to a
function with tens of thousands of instructions. Root-caused via bisection
across the five `.disto` commits + `git worktree` isolation, then fixed in
three parts:
1. **Symbolic redundancy removed** (`crates/piperine-codegen/src/lower/diff.rs`):
   `d_dv_once_more_named`/`d_dv_thrice_from_twice` complete an
   already-built first/second derivative pass with one more differentiate
   step, instead of `compile_disto2`/`compile_disto3` redoing the first
   one/two passes from the raw expression for every branch pair/triple.
2. **One Cranelift function per branch combination**, not one function
   unrolling every combination (`crates/piperine-codegen/src/jit/analog.rs`)
   — `AnalogKernel::disto2`/`disto3` are now `Vec<AnalogFn>` (one entry per
   `disto2_pairs`/`disto3_triples` index), each writing its own `nc`-sized
   output slice.
3. **`compile_disto: bool` flag** threaded `AnalogKernel::compile_with_options`
   → `CompiledModule::compile_with_options` → `CircuitCompiler::with_disto`
   → `SimSession::build_circuit` (`crates/piperine-api/src/session.rs`) —
   every `run_*` analysis but `run_disto` passes `false`, skipping the
   `.disto` kernel compile entirely (its cost is real even after fix 1/2,
   and only `.disto` itself needs it). Existing direct `AnalogKernel::compile`/
   `CompiledModule::compile` callers (codegen/lang test fixtures) are
   unaffected — those keep the `compile_disto: true` default.

Also added (needed to make fix 2 tractable at all): `Cargo.toml` dev-profile
`opt-level = 3` override for `cranelift-codegen`/`cranelift-jit`/
`cranelift-module`/`cranelift-frontend`/`cranelift-native`/`regalloc2` —
Cranelift's own register allocator is prohibitively slow unoptimized, and
every analog kernel compile (not just `.disto`) now benefits. This made
`examples/live_optimize.py`'s fresh-build path faster in absolute terms,
shrinking its `>= 10x` speedup ratio against the live-restamp path to
~7.9x (MD-18's real invariant — zero recompiles in the live loop — is
unaffected, checked separately via `compile_count`); threshold lowered to
`>= 5x` with a comment explaining why.

**Result:** full `tests/ngspice_validation.rs` suite (30 tests, includes
MOS2/MOS3 op-points that previously hung indefinitely): infinite → 370s
(fix 1+2 alone) → **5s** (+ fix 3, the flag). `cargo test --workspace`:
582 passed, 0 failed, 5 ignored.

**Next for this feature:** none — delivered. If further `.disto` perf is
ever needed on very-many-branch devices, the next lever is Schwarz-symmetry
deduplication (mixed partials are order-independent — cuts branch-pair/
triple combinations by ~2×/~6× before compiling), not attempted here
(diminishing returns given fix 3 already makes the cost opt-in).

### Feature — `solver-simplification` (DELIVERED 2026-07-19, Verifier PASS)

Spec/design/tasks in `.specs/features/solver-simplification/`.
Behavior-preserving refactor of `piperine-solver`; the oracle is the P0
parity baselines (bit-identical) plus the unchanged 520-test suite.

- **Batch 1 (P0+P1)** ✅ — parity baselines pinned; dead surface removed
  (`LINEAR`, `ANALYTIC_JACOBIAN`, `STAMPS_CHARGE` + producers/asserts,
  phantom rollback doc); `SignalBridge` folded into `CircuitInstance`.
- **Batch 2 (P2+P3)** ✅ — `math/unit.rs` removed (`f64` inline, `Second`
  off the ABI surface); config home `analyses/config.rs`
  (`GminSchedule`/`SourceSchedule`/`StepperGains`/`TraceFlags`, defaults
  == former literals) wired into homotopy, `PiController`, trace path.
- **Batch 3 (P4)** ✅ — `Element` = `AnalogDevice + DigitalDevice +
  Introspect` conjunction (MD-01 amended 2026-07-19); codegen
  `PiperineDevice` + test doubles regrouped into the four blocks;
  composed-surface contract test (`composed_element.rs`).
- **Batch 4 (P5+P6)** ✅ — `CircuitInstance` five contracted sections;
  `solver/` + `analysis/` collapsed into `analyses/` (Scheme B, data +
  driver co-located); per-module `//!` layer contracts.
- **Batch 5 (P7+P8)** ✅ — transient `solve()` decomposed into named
  phase methods (`predict_step` / `attempt_step` / `assess_step` /
  `accept_step` / `settle_digital` / `record_step` / `propose_dt` /
  `reject_lte_step` / `reject_step`, plus `begin_run` / `finish_run` and
  the `TimeLoop` state struct — no driver method > 60 lines); STATE.md
  refreshed (MD-05 done, MD-01 amendment, this snapshot); module `//!`
  contract audit.

- **Batch 6 (P9)** ✅ — T33 `209be1c` Part VII §2–§5 rewritten (five
  contracted `CircuitInstance` responsibilities, composed supertraits +
  full `ElementCapabilities` table) · T34 `d5e54a2` §8–§16 stated-vs-code
  audit · T35 `70395f2` consistency + completeness (§17 sens, §18 PSS,
  failure rows, zero phantom references). Docs-only; no code touched.

**Close:** all 35 tasks done (`tasks.md` Progress Log, batches 1–6). The
Verifier returned **PASS — 18/18 requirements covered**, sensor 3/3 killed
(`.specs/features/solver-simplification/validation.md`, diff range
`4565f9e^..dc4fa07`). At close: `cargo test --workspace` 520 green /
5 ignored, 0 rustc warnings; parity baselines bit-identical through every
batch. **No residue.**
**Branch:** `feature/bench-removal`.

### Previously delivered features (summary)

- **`p1-solver-complete`** (DELIVERED 2026-07-18, Verifier round 2 PASS) —
  25/25 active ACs, sensor 6/6; ROADMAP pillar P1 closed. Details in
  `.specs/features/p1-solver-complete/validation.md` and git history.
- **`bench-removal`** (DELIVERED) — in-language `bench` gone; root
  `piperine` crate is the library face (MD-19, superseded by MD-20);
  tests of record in root `tests/`; `piperine test` runs `*_tb.py`.
- **`solver-trbdf2-engine`** (DELIVERED) — TR-BDF2 sole scheme, PI
  controller always-adaptive, unified analog/digital breakpoints.
- **`python-bindings`** (DELIVERED) — `piperine-python` (PyO3) +
  pure-Python facade; PY-01..PY-17 verified.
- **`solver-convergence-performance`** (DELIVERED) — `SolverStats` wired,
  zero-alloc Newton, device bypass, `ConvergenceHint`, Tolerances/Policy
  split (MD-04 done).

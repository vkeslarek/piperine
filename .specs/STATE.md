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


### MD-52: digital vectors are scalarized per bit

`lrm-conformance-wave3` design decision (2026-08-27, user-confirmed).
A width-N digital net (`reg [3:0] q`, `wire [7:0] bus`, a memory
`reg [3:0] mem [0:7]`) lowers to **N independent `Quad` nets** named
`q[0]`..`q[N-1]` (`mem[d][b]` for a memory), exactly as an analog array net
already does (`resolve/pom/structure.rs`'s element-node loop). `q[i]` resolves
to one net; a part-select resolves to an ordered chunk list; a whole-vector
read or write expands to per-bit operations emitted in declared bit order, in
the region the assignment already selected (`=` Active, `<=` Nba).

**Why not packed.** A packed word (2 bits per 4-state bit in an `i64`) needs a
new value class in the digital ABI, `SigRef`-style chunk plumbing through
`emit`, and per-word rework of `DriverTable`, `layout`, `compile` and
`network` — for a runtime win nothing currently asks for, and a 32-bit width
ceiling. Scalarizing changes **no ABI**: driver resolution by strength,
the stratified regions, checkpoint/rollback and `$driver_*` all keep working
untouched.

**Known cost, accepted:** net count grows with width × depth (a `[3:0]` × 8
memory is 32 nets). A width×depth cap fails loud rather than exhausting
memory. The packed representation stays the documented future optimization —
`piperine-ir` already models it (`Ty::Bits(N)` + `SigRef` chunk lists), so
adopting it later is a backend change, not a language change.

**Status:** Locked (user, 2026-08-27).

---

### MD-53: the superset surface is gated the same way the LRM surface is

`lrm-conformance-wave3` design decision (2026-08-27, user-confirmed).

**Status (2026-11-12):** retired. `docs/spec/` was replaced by the MSL
specification and the axis-B gate (`spec_section_coverage.rs`, `SPEC-COVERS`)
was deleted with it in 72181d1a (user decision). Axis A (LRM ↔ impl) stands.
Piperine is a **superset** of Verilog-AMS, so conformance has two axes: LRM ↔
impl (MD-51's `LRM-COVERS` gate) and **Piperine's own spec ↔ impl** — the
`constraint`/`tol`/host-API/plugin/POM/IR surface the LRM never mentions,
which had no gate at all.

Axis B copies MD-51's shape rather than inventing a second one: a test
declares what it proves with `//! SPEC-COVERS: <ids>` (ids are
`<spec file stem>#<heading number>`, e.g. `part_viii#15.2`), and
`tests/spec_section_coverage.rs` **derives** the matrix by parsing
`docs/spec/*.md`'s own headings. Every heading must resolve to `Covered`,
`DeclaredLimitation { reason }` (an honest documented gap, e.g. "Rust-only,
no Python binding"), or `Informative { reason }`. A spec claim added without a
test turns the gate red; a declaration naming a heading that does not exist,
or living in a file with no live test, turns it red too.

Three rules travel with it (spec §0):
1. An LRM clause Piperine deliberately does not implement is an exclusion
   **naming its replacement**, never a defect.
2. A clause implemented differently on purpose is a deviation with a pinning
   test.
3. An extension is **not** exempt from proof — it needs the same live test an
   LRM clause needs.

**Status:** Locked (user, 2026-08-27).

---

### MD-54: the MSL POM is an arena model built once; only lowering creates nodes

`msl-pom` design decision (2026-11-13). The Piperine Object Model of the MSL
frontend is `piperine_lang::pom`: one owning, immutable `Design` holding arenas
addressed by typed ids (`DeclId`, `ExprId`, `StmtId`, `PatId`, `TypeId`,
`PathId`, `FileId`); the public face is `Copy` handles `(design, id)` in the
style of Java reflection. It is the concrete form of the navigability rule for
MSL: the tree mirrors the authored code, bodies included, and converts back to
the syntax tree (`to_ast`).

Three rules travel with it:
1. Only lowering (AST → arenas) creates nodes. Name resolution, classification
   and every later analysis write into slots or side tables keyed by the ids.
2. Elaborated forms (unrolled generation, monomorphized modules, instance
   copies, the flat netlist) are separate artifacts keyed by POM ids, never
   edits of the `Design`.
3. The `Design` and every storage type derive `Serialize`/`Deserialize` (user,
   2026-11-13: "Derive já"). The retained syntax tree is not serialized; it is
   rebuilt from the retained source text.

Trade-off: a second tree beside the AST (lowering plus the reverse conversion)
instead of an `Rc` object graph or AST-valued bodies with span-keyed tables;
chosen for `Send + Sync`, plain serde and stable ids for the type checker.

**Scope:** `piperine-lang` and every consumer of the frontend (type checker,
elaborator, codegen, LSP, host API).
**Status:** Locked (msl-pom spec approved 2026-11-13).
(rule 3 is already the user's decision).

### MD-55: MSL source files are `.msl`, and `mod` follows Rust's on-disk rules

User decision (2026-11-13: "Regras do Rust no disco"). `mod b;` loads `b.msl`
or `b/mod.msl` relative to the declaring file, exactly as rustc resolves
`b.rs` / `b/mod.rs`; the loader lives in `piperine-lang` (`pom::source`) behind
a `SourceProvider` with a disk and an in-memory implementation, so one rule
set serves files, tests and unsaved editor buffers. A `mod` that cannot be
loaded (no file, two candidates, a cycle) is `E0161` (§3.4). `#[path]` has no
meaning: it is not an attribute of the language (Appendix B.1).

The extension `.msl` is the planner's default: `docs/spec/` names none.

Trade-off: `piperine-lang` reads the filesystem, instead of the host
registering each source under a namespace path.

**Scope:** `piperine-lang`, `piperine-project`, the CLI and the LSP.
**Status:** Locked (user, 2026-11-13); the extension is open to correction.

### MD-56: the MSL audit's language decisions (D1 to D17)

User decision (2026-11-14: "sim"), accepting the seventeen recommendations of
`.specs/features/msl-audit/report.md` ("Spec decisions needed") as written.
Feature `msl-polish` writes them into `docs/spec/` and brings the parser and
the POM into line; D4 is the next feature, `msl-stdlib`.

- **D1** B.3.6 is the complete list of what is removed from Rust (type aliases,
  associated types, trait-bound `where`, `impl Trait`, qualified paths,
  `pub(..)`, labels, pattern and `mut` parameters, `&self`, ...). Taken in:
  function-pointer types `fn(T) -> U`, a negative literal as a const generic
  argument.
- **D2** The inherited Rust library is a closed catalogue (Appendix H); a name
  outside it is `E0161`.
- **D3** `gnd`, `Nature`, `m`, `dtemp` and `str` are core names (built-in table),
  not prelude source.
- **D4** The standard library is a normative source (`std/*.msl`), Part VII's
  fences are excerpts of it, and "the galleries build clean" is a gate.
  **Deferred to `msl-stdlib`.**
- **D5** One name per table (type, value) per namespace; an item or explicit
  `use` beats a glob; two globs clash only when the name is used. Code `E0162`.
- **D6** A behavior block repeats its module's generic names, same names, same
  order.
- **D7** A shared borrow of any value is allowed as a call argument; `E0102`
  covers the rest.
- **D8** `E0910` counts nesting only (256). Flat chains have their own bound:
  4096 links on one path (planner default inside the decision).
- **D9** Doc comments and attributes go wherever Rust takes an outer attribute,
  and before reactions, definitions, generation and nature quantities. Docs of
  a quantity and of a reaction are metadata (§21).
- **D10** A plain identifier pattern that resolves to a constant, a unit variant
  or a unit struct is that path, not a binding.
- **D11** The POM reports the structural errors it can already see under the
  spec's codes (`E0320`, `E0140`, `E0370`, `E0201`; by the same rule `E0263`,
  `E0125`, `E0206`, `E0211`).
- **D12** An out-of-range literal is a parse error, `E0909`.
- **D13** Items sit at namespace scope (a file or an inline `mod`); a block and
  its module share a file, any namespace of it.
- **D14** `E0404` for `<-`, `+>`, `on`, `signal` in any block or body that is not
  theirs, a module body included; `E0121` only in a `fn` body, where it wins.
- **D15** `r#name` is an identifier for the seven added keywords (closes G.15).
- **D16** `{ r }` is accepted in param blocks and templates and the tree keeps
  the authored form (closes G.14).
- **D17** A place takes no parentheses, in a definition or in a commit.

Trade-off: the language is narrower than "Rust minus a few things" on paper
(B.3.6 and Appendix H are closed lists that must be kept), in exchange for a
grammar and a library surface a type checker and a conformance suite can be
written against.

**Scope:** `docs/spec/`, `piperine-lang` (`msl`, `pom`), `docs/pom/`, and every
later consumer of the frontend.
**Status:** Locked (user, 2026-11-14). D4 open until `msl-stdlib`.

### MD-57: a spec edit lands where its guard can pass; `spec_text` guards the wording

`msl-polish` design decision (2026-11-14). `docs/spec/` is normative and is
read by tests, so the specification leads and every commit stays green:

1. Prose, rules, `Resolution` notes and elaboration-phase error rows are
   written first, each with a test in `crates/piperine-lang/tests/spec_text.rs`
   that asserts the decided fact (a row, a code, a short phrase; never a
   sentence).
2. Three kinds of edit travel with their implementation, because an existing
   guard reads them: an error row of phase `parse` (its fixture, `msl_errors`),
   a `rust` or `rust,fragment` fence (`msl_corpus`, `pom_corpus`), a new
   production name of B.3.2 (`docs/pom/mapping.md`, `pom_docs`).
3. No guard is relaxed to let a spec edit through.

Trade-off: for the length of a feature the specification can be ahead of the
implementation; the alternative (spec and code in one commit per decision)
makes every task span four layers.

**Scope:** every feature that edits `docs/spec/`.
**Status:** Locked (msl-polish spec approved 2026-11-14).

### MD-58: packages follow Cargo; the front end takes a crate graph as data

`msl-packages` design decision (2026-11-14).

A crate reaches another only through a dependency named in `Piperine.toml` (user, 2026-11-14: "faremos modelo RUST 100%"); there is no search path. A local name that equals a dependency name wins in an ordinary path and is an ambiguity error (`E0163`) in a `use` path (user, U6). `piperine-lang` takes a `CrateGraph` (names, root files, named edges, acyclic by construction) and never reads a manifest or a repository; `piperine-project` owns `Piperine.toml`, path and git resolution and `Piperine.lock`, and nothing else. One `Design` holds every crate as its own tree (navigability rule); a dependency name resolves to the root namespace of the other crate. Trade-off: no include path, ever, and a `Design` whose serialized form carries crates. Scope: `docs/spec` §3.5 and §3.6, `piperine-lang`, `piperine-project`, and every host.

**Status:** Locked (msl-packages approved 2026-11-14).

### MD-59: `std` is a crate embedded in `piperine-lang`; the prelude is `std::prelude`

`msl-packages` design decision (2026-11-14).

(user, 2026-11-14: "std fica embutida o binário"). Its sources are `crates/piperine-lang/std/*.msl`, compiled in with `include_str!`; every crate depends on it implicitly; the builder's prelude sources are gone (`std_source` replaces the library in tests). Supersedes the "prelude sources" of MD-54's builder and closes the open half of MD-55 (the extension `.msl` is now in §3.6). Scope: `piperine-lang`, `msl-stdlib`.

**Status:** Locked (msl-packages approved 2026-11-14).

---

### MD-60: the solver is three crates; events are the only boundary

`solver-restructure` (2026-11-14; numbered 60 because MD-58/MD-59 are taken by `msl-packages`/`codegen-plan`). The solver
is three crates: `piperine-solver` (the owner: analyses, the `Element` ABI,
event dispatch), `piperine-analog-solver` and `piperine-digital-solver`. The
owner depends on both sub-solvers; the sub-solvers depend on neither the owner
nor each other, and no in-tree crate sits under any of them.

1. **Events are the only boundary** between analog and digital; this is the
   target of the follow-up feature `solver-event-boundary`. User: "A fronteira
   é via eventos, sempre... Separar em uma fronteira BEM delimitada o que
   ocorre no analog e o que ocorre no digital, praticamente como dois solvers
   distintos que rodam no mesmo solver."
2. **Three crates.** User: "o dono dos eventos, o digital-solver e o
   analog-solver cada um com seu código exclusivo."
3. **Opaque byte carriers** cross the boundary. User: "Prefiro bytes também.
   Solver é o core do negócio e tem que ser mais simples, não ter a
   complexidade da linguagem e sim a complexidade física."
4. **The present discontinuity algorithm and tolerance mapping are kept.**
   User: "eu manteria o algoritmo atual do solver que funcionou bem nos
   testes, inclusive cross validation e convergence com descontinuidades" and
   "a paridade com ngspice é um strenght bem alto". Spec amendments owed to
   `solver-event-boundary`: Appendix C.2, section 17.1.2 and section 15.2 are
   amended to describe the kept algorithm, not the reverse.
5. **Five crates removed:** `piperine-ir`, `piperine-codegen`,
   `piperine-python`, `piperine-plugin`, `piperine-plugin-macros`, with the
   root VAMS end-to-end tests and `examples/*.vams`. User: "Vamos remover o IR,
   cranelift e Python por agora" and "plugin pro enquanto vai embora".
6. **Kept but not building:** `piperine-project`, `piperine-api`,
   `piperine-cli`, `piperine-lang-server`. User: "lang-server é bom ficar...
   cli também mas ambos vão ter sua própria feature."
7. **Unit tests only for now.** User: "por enquanto vamos ter que nos
   contentar com unitários pois integrado só vai existir depois que
   conectarmos o solver ao lang." The hygiene guard moves into
   `piperine-lang` tests ("Move guard, delete py").

**Scope:** `crates/piperine-solver`, `piperine-analog-solver`,
`piperine-digital-solver`, the workspace manifests and the instruction files.
**Status:** Locked (2026-11-14).

### MD-61: `piperine-abi` sits under the three solver crates

`solver-abi-core` (2026-11-15). The device ABI (`.specs/features/device-abi/abi.md`)
lives in a new crate `piperine-abi`: the `#[repr(C)]` contract, the Rust `Device`
trait with its lenses and thunks, the solver-side `caller::Handle`, shape-local
`validate` and the partials verifier. It depends on no crate at all. Each solver
crate may depend on it, and it is the only in-tree crate a sub-solver may depend on.
This amends MD-60's "no in-tree crate sits under any of them".

1. **One crate holds the contract.** User (through the coordinator, 2026-11-15):
   "new crate `piperine-abi`", "digital sub-solver on the ABI", "analog sub-solver on
   the ABI". Every device (test devices, the interpreter, generated Rust) links against
   it and nothing else.
2. **The sub-solvers still never see each other.** `piperine-abi` is vocabulary only:
   no solver logic, no dependency, so it carries no path from one sub-solver to the
   other. The guard `crate_boundaries::sub_solvers_depend_on_the_abi_crate_only`
   allows exactly this crate and still catches a dependency on the other sub-solver;
   `abi_crate_is_a_leaf` keeps it dependency-free.
3. **Trade-off.** A change to the ABI recompiles all three solver crates and every
   device; in return the C layout, the call path (`caller`) and the checks
   (`validate`, `verify_partials`) exist once, and the only `unsafe` of the call path
   lives in `raw.rs`, `thunk.rs` and `caller.rs` of this crate.

**Scope:** `crates/piperine-abi`, the manifests of the three solver crates, the
structural guards (`crate_boundaries.rs`, `workspace_shape.rs`), `CLAUDE.md`,
`AGENTS.md`.
**Status:** Accepted (2026-11-15), under the user's go-ahead "Ok, deixo tu ir tocando, pode ir
disparando implementações em seguida ok?" as recorded in
`.specs/features/solver-completeness/context.md`, "Orchestrator decisions under the user's
go-ahead" (row MD-61).

### MD-62: the device ABI is the only device contract

`solver-abi-integration` (2026-11-15). The owner `piperine-solver` drives every
analysis through the device ABI of `piperine-abi`: the analog sub-solver's
`Assembly`, the digital sub-solver's `Digital` and the owner protocol
(`src/protocol/`, with its adapters in `analyses/adapters.rs`) over the two. The
`Element` trait, `AnalogDevice`, `DigitalDevice`, `Introspect`, the capability
flags, device checkpoints, `LimitingReport`, `Invalidation`, `next_breakpoints`, the
strength table, `LogicValue`, `DigitalState`, `EventQueue` and the two-phase
engine are deleted; `tests/removed_surface.rs` guards each name and
`removed.txt` lists every removed test with its item.

1. **The solver owns what a device used to keep.** The companion (a `DDT` term's
   history and its TR-BDF2 stamp), the limit iterate (a `LIMIT` input's held
   state), every history and every checkpoint (a rejected attempt restores both
   sub-solvers and the sink, abi.md §5) are the assembly's and the protocol's, not
   a device's. A device is `eval` over declared reads, writes and partials, and
   `react`.
2. **Boundary delivery.** At an instant the digital side changes a value the
   analog side reads (or the host writes a param), the step lands on the instant
   solved on the value before it (the left value); then the instant runs; then the
   change is delivered with a restart (`Restart::Integrator`: `prev_h = 0`, the
   next step exempt from the truncation test; `IntegratorSmall` after a host param
   write: also a step of 1e-3 of the last, floored at `dt_min`). User (2026-11-15,
   through the coordinator): solve on the left value, then react. This supersedes
   the boundary preview of `solver-abi-core` and `live_params`' former reading
   that the landing already shows the new value.
3. **One circuit kind.** `CircuitBuilder::build` always binds; a circuit has no
   second contract and no mixed form. `CircuitInstance::set`, `schedule_set` and
   `on_event` are the host's actions; `SolverDomain::Device` replaces
   `SolverDomain::Element`.
4. **Known gap.** A device transient with a non-zero start and no captured step
   fails loud (digital init only at tick 0); `ROADMAP.md` P1 backlog.

**Scope:** `crates/piperine-solver`, `piperine-analog-solver`,
`piperine-digital-solver`, `CLAUDE.md`, `AGENTS.md`.
**Status:** Accepted (2026-11-15), under the user's go-ahead "Ok, deixo tu ir tocando,
pode ir disparando implementações em seguida ok?" and the settled boundary order above.

### MD-63: the compiled backend is `piperine-transpile`; the shared plan is `piperine-plan`

`ir-rust` (2026-11-15). The production path from the IR to the solver is Rust:
`piperine-transpile` prints a design as one Rust source text, builds it with `rustc`
into a `cdylib` exporting `piperine_abi_1`, caches it by hash and loads it. The plan of
a body (its ABI shape and what each function computes) and the circuit of a design live
in `piperine-plan`, which both the interpreter and the transpiler read, so the two
backends give the solver the same pins, read order, partial order and affine flags.

1. **Generated code links `piperine-abi` and nothing else.** Besides that crate it uses
   only the embedded runtime source `rt.rs`; it holds no `unsafe` and no macro.
2. **`rustc` is invoked directly and only at build.** Never `cargo`; never during an
   analysis or a `set` (C7, MD-18). A missing compiler fails the build and leaves the
   interpreter usable.

**Scope:** `crates/piperine-plan`, `crates/piperine-transpile`, `crates/piperine-interp`,
`CLAUDE.md`, `AGENTS.md`.
**Status:** Accepted (2026-11-15), under the user's standing delegation "Pode ir rodando os
grandes passos" (`.specs/features/codegen/context.md`, R4) and the approved
`.specs/features/ir-rust/design.md`.

### MD-64: result modules are hoisted in the IR; unknown-ness is an elaboration analysis; a record's text stays in the IR

`msl-digital` (2026-11-16). Digital and mixed MSL reach the IR node set of `ir-rust` from source,
and three choices fix how.

1. **A module with a result is hoisted.** Its signals, drives, events and reactions join the
   body that places it, named under the placement (`gate.x`, `above$0.init$0.up`); its result is
   inlined at each read with its value inputs substituted (part III §8.3.12, Appendix C.1.1). It
   gets no IR body and no instance. The POM keeps the placement as written (the navigability
   rule): hoisting exists only in the IR.
2. **Unknown-ness is computed at elaboration, not typed.** The checker gives the source's own
   types (`count + 1` is `u9`); the elaborator's `Known` analysis gives an expression that may be
   unknown the IR type `Flagged` (§3.3: the flag travels with the value, it is not part of its
   type). A function is lowered once per flag pattern of its arguments.
3. **The text of a record stays in the IR.** A `&str` field of `emit!` is a literal kept in the
   IR's emit site, never on the ABI, which has no string leaf; a host ties a delivered record to
   its texts with `Body::record_site(reaction, statement)`, keyed by the commit observation's
   reaction and statement (abi.md C5).

**Scope:** `crates/piperine-lang` (`types/`, `elab/`), `crates/piperine-ir`, `crates/piperine-plan`,
`crates/piperine-transpile`, `CLAUDE.md`, `AGENTS.md`.
**Status:** Accepted (2026-11-16), under the user's standing delegation and the approved
`.specs/features/msl-digital/design.md` ("Tech Decisions").

---

### MD-65: the command line is a shell over `piperine-api`; a run is a `[run.<name>]` table

`msl-cli` (2026-11-17). The command line and the host library run an MSL project end to end.

1. **The command line is a shell over `piperine-api`.** `piperine-cli` parses arguments (clap's
   builder, no derive), calls the library and lays out text; every rule of what a run prints or
   judges (which signals, rows and records, pass or fail, every validation) is a `piperine-api`
   method with a Rust test. The old content of `piperine-api` is gone and each of its features
   returns on the device ABI under its own spec (amends MD-20's module list; its topology, the
   library face with the root package re-exporting it, stands).
2. **A run is a `[run.<name>]` table of `Piperine.toml`** whose keys are also flags of
   `piperine run` (a flag wins); a lone `.msl` file runs from flags. `piperine-project` accepts
   and locates the table and does not read it (amends MD-58's "and nothing else");
   `piperine-api` reads it.
3. **A bench is a crate under `benches/`**, named `bench`, depending on the package and on its
   direct dependencies: a bench is MSL (stimulus and monitors), and the spec says so; sweeps,
   Monte Carlo, optimization and mid-run stimulus stay in the host.
4. **`msl::format` is the formatter, `msl::print` stays the canonical text of a tree.** The
   formatter keeps every plain comment (anchored by span from a side list of the lexer) and
   refuses to return a text that loses one.
5. **The language server is deferred and untouched** (user, 2026-11-16), outside the default
   build.

**Scope:** `crates/piperine-api`, `crates/piperine-cli`, `crates/piperine-project`,
`crates/piperine-lang` (`msl/{comments,format,print}.rs`), `docs/spec/`, `docs/book/`, `examples/rc`,
`examples/buck`, `CLAUDE.md`, `AGENTS.md`.
**Status:** Accepted (2026-11-17), from the approved `.specs/features/msl-cli/design.md`
("Tech Decisions") and the user's eight answers of 2026-11-15.

---

### MD-66: observation is the run's, not the language's

`long-run` (2026-11-17). A long run keeps rows, not steps.

1. **Observation is the run's.** A transient hands each accepted step to a host observer, and a
   host keeps rows, never steps: a session records the signals it prints, on its print grid as
   the steps arrive, and the solver keeps its last step only (`Keep::Last`); the solver's own
   default result still keeps every step (N2).
2. **A new observation option is a key of `[run.<name>]` with its flag**, as MD-65 item 2 says of
   every run option; no key or flag was added by this feature.
3. **Measurements over a waveform are host code**: overshoot, settling and ripple are computed in
   a host test over the run's rows, never in the language.

It extends MD-65 item 1 and supersedes nothing.

**Scope:** `crates/piperine-solver` (`protocol/step.rs`, `Keep`), `crates/piperine-api`
(`recorder.rs`, `Session`), `crates/piperine-cli`, `crates/piperine-parity` (`parity_grid`),
`examples/buck`.
**Status:** Accepted (2026-11-17), from the approved `.specs/features/long-run/design.md`.

---

### MD-67: the inherited library is the closed catalogue of Appendix H, as IR `Method` rows

`msl-fixes` (2026-11-18). The correctness fixes and the inherited library.

1. **The inherited library is the closed catalogue of Appendix H**, implemented as IR `Method`
   rows on the Rust backend with Rust's own semantics (H rule 3: Rust's `f64` and integer methods
   are the oracle). A method outside the rows of a complete receiver type (`f64`, an integer,
   `Option`, `bool`) is `E0161` naming the method and the type; the carrier methods of B.1 and the
   names a project defines stay `P0001`. `exp`, `ln`, `sqrt` and `abs` of `f64` stay `MathFn`, so
   the twins' generated text does not move.
2. **An `Option` is the pair `(is, val)` in the IR**, and an `Option` parameter two parameters; a
   closure argument of an `Option` row is inlined at the call, `checked_*` is a wrapping op plus an
   overflow test, and `unwrap` is `rt::Unwrap::of`.
3. **Unknown in a `CROSS` watch is NaN, and NaN is unknown, not a fault** (Q2): the watch's `cross`
   memory resets, no crossing fires at the next solution, and only infinity fails. This amends
   `abi.md` A4 ("non-finite") for a `CROSS` watch.
4. **A run ends on a `sim` record with a known `finish = true`; `fatal` ends it with
   `failed = true`** (X4): `piperine run` exits 1 and `piperine test` reports `FAIL` with the
   message. The solver already had the mechanism; the host reads it from the record stream.
5. **An AC or noise operating point carries its analysis's flag** (Q3): `DcSolver::under` takes
   the analysis global; `analysis(Dc | Transient | Ac | Noise)` reads it, `freq()` is the `FREQ`
   ambient in `analog` and 0.0 in a reaction, and `bound_step` is one `CAP` watch per top-level
   site (a conditional one is `P0001`, Q4, until L40).
6. **S3 (the type checker reaches only what a run's top reaches) is not part of it** (Q1): it comes
   with X6, in a feature of its own.

It supersedes nothing and extends MD-64 (the IR's value forms).

**Scope:** `crates/piperine-lang` (`types/`, `elab/`), `crates/piperine-ir`, `crates/piperine-plan`,
`crates/piperine-transpile` (`rt/`, `print/`), `crates/piperine-interp` (refusals only),
`crates/piperine-analog-solver` (`device/watch.rs`), `crates/piperine-solver` (`analyses/dc.rs`,
`ac.rs`, `noise.rs`), `crates/piperine-api` and `crates/piperine-cli` (the run's end and
`fatal`), `crates/piperine-parity` (`source_*`), `docs/book`.
**Status:** Accepted (2026-11-17), from the approved `.specs/features/msl-fixes/design.md`.
---

### MD-68: run errors are typed in the solver and named in the owner; re-entry is exact; the numerics follow ngspice

`solver-hygiene` (2026-11-18). Lane C of the 2026-11-17 plan; the user's answers, all "Aceito todas
(Recommended)" (Q0 to Q7), bind this entry. (Numbered MD-68: lane A's `msl-fixes` holds MD-67.)

1. **A run error is typed in the solver and named in the owner.** `piperine-analog-solver` raises
   `E0503` (singular, non-finite solution), `E0505` (an equilibrium that never converges), `E0507`
   (a non-finite contribution or stamp), `E0508` and `E0504` (a step that fails at the floor, or
   reads a jump), each with the unknowns it concerns; the owner (`CircuitInstance::name_error`)
   turns unknowns into `node of path.pin` and `flow of path (pin)`. No error cites an LRM section
   for a spec code. A singular diagnosis is skipped, and says so, above 2 000 unknowns.
2. **Re-entry is exact.** A transient re-entered from a recorded step continues the run bit for
   bit: the re-entry state carries the Newton history, the step controller's memory (`prev_error`),
   `prev_h` and the pending restart. `Reentry::LastStep` is the opt-out (N2): only the last step
   carries a state. User: "Aceito todas (Recommended)" (HYG-04, HYG-13).
3. **The truncation test measures the `ddt` states with ngspice's `CKTterr` tolerance.** Q1(a):
   `chgtol` is read, the PI target 1 and the gate `trtol` 7 are unchanged. Q2(a): the asymmetric
   Milne skip is removed. Q0(a): a `ddt` whose argument is nonlinear in a `limit`ed value no
   longer jumps from its operating point (HYG-05).
4. **The step floor.** Q3, amended 2026-11-18 by the user ("Seguir o ngspice exato"): ngspice's
   own rule (`dctran.c`, `olddelta`). A failed attempt whose smaller step is at or under `dt_min`
   is retaken at `dt_min` once; Newton failing at an attempt already at the floor is `E0508`
   (`E0504` after a delivery); a step at the floor that fails only the truncation test is accepted
   and counted (`floor_accepts`), as ngspice completes those circuits. Was: a retry, then `E0508`.
5. **Newton damping.** Q4(a): no damping by default; `NodeDamping` (ngspice `niiter.c`) is opt-in;
   the DC and transient drivers iterate with the plan's `NewtonStrategy`.
6. **The step after a jump.** Q5(a): at most `0.1·min(last step, gap to the next scheduled time
   point)`, first order, exempt from the truncation test; a host `set` keeps its 10^-3 restart.
7. **A later start.** Q6(a): a transient with `start_time` ≠ 0 and no captured step solves the
   operating point at `(t_start, 0)` (the digital engine starts at that tick); a non-finite or
   negative start fails first. Q7(a): `failure_rules.rs` asserts `E0507`; `boundary.rs` runs at
   the 10 ns cap.
8. **The digital checkpoint is a journal.** `Store` and `Queue` record what changes under a mark
   and `rewind` undoes it; the protocol marks the digital side that way and copies the state only
   where a re-entry state is recorded.

Tolerances did not move: `crates/piperine-parity/src/contract.rs` is unchanged, and the goldens
that moved (`diode_series`, `parity_rc_transient`, `parity_diode_dc_point`) did so under the
deviation tables of their commits.

**Scope:** `crates/piperine-analog-solver`, `crates/piperine-digital-solver`,
`crates/piperine-solver`, `crates/piperine-parity`, `docs/spec` (Part V, Appendix C).
**Status:** Accepted (2026-11-18), from the approved `.specs/features/solver-hygiene/design.md`.

---

### MD-69: a host `set` recomputes the params derived from it, through generated functions

`host-ii` (2026-11-18). The host reaches what the solver already does.

1. **A host `set` recomputes every value derived from the param it changes** (§8.1.23, decision
   X9): the child params, the constant `let`s and the definitions whose expression reads it. The
   derivation is a function the compiled backend generates, `piperine_derive_1(k, params) -> f64`,
   from a `DerivePlan` that `piperine-plan` builds from the IR; the host calls it, then writes each
   changed pin through `CircuitInstance::set`, checking each child's `where` first and undoing
   the writes on a refusal (§23.1 rule 6). A timed `set` derives when it is scheduled, one write
   per changed pin at the same time. This closes S2 of `GAPS.md`.
2. **MD-18 stands unchanged.** Elaboration fixes the devices and the circuit compiles once; a
   `set` restamps the compiled circuit and neither re-elaborates nor recompiles. The interpreter
   backend has no derive function, so a param with readers is refused there naming
   `--backend rust`.
3. **The host drive and the timed `set` are a library surface** (X8, Q2): `Session::drive`,
   `Session::set_at` and `Session::subscribe` of `piperine-api`; the command line has neither a
   key nor a flag, and a bench in MSL stays its stimulus. A session's options are run keys with
   ngspice's `.options` names (X7), and `piperine test` reports a `target` and never fails on it
   (X3).

It supersedes nothing and extends MD-18 and MD-65.

**Scope:** `crates/piperine-plan` (`DerivePlan`), `crates/piperine-transpile` (the derive export),
`crates/piperine-api` (`params.rs`, `Session`), `crates/piperine-solver` (`CircuitInstance`
writes), `crates/piperine-cli`, `docs/book`.
**Status:** Accepted (2026-11-17), from the approved `.specs/features/host-ii/design.md`.

---

### MD-70: the unknown-value rules run in generated code; the ABI grows one entry, `commit_with`

`unknown-semantics` (2026-11-19). Part I §3.3, §1 rule 13 and §22.1 on the compiled backend.

1. **The merge of §3.3.3 runs in the generated Rust**, one rule for every owner of code: a
   control on a value that may be unknown (an `if`, a `match`, a guard, a short-circuit
   operand) runs both paths and merges each effect, so the fault of an unknown `if` is gone and
   a panic on a path under unknown control makes the merge unknown (§1 rule 13). `level`,
   `merge`, `same`, `cast`, `logic` and `matches_bits` are typed and lowered by their §22.1
   contract at the concrete type, in `elab/intrinsics.rs`, which is deleted when `generics`
   compiles `std/values.msl` (Q1 (b)); a loop under unknown control is bounded by 64
   iterations, written in Appendix C.2.
2. **The device ABI grows one additive entry**: `Host.commit_with(.., flags)` with
   `commit_flag::{MAYBE, MERGE}`, `MINOR` 1, abi.md C7. Transport converts under `MAYBE`; a
   `MERGE` commit merges by layout (`layout::Merge`). No `Statement`, `Pin` or layout changed.
3. **A record carries `maybe`**: every stream's layout ends with a known `bool` field, true when
   the `emit!` was reached under unknown control; a `fatal` record that is `maybe` does not end
   the run (`Record::surely`), and a `maybe` check is judged as unknown.
4. **`Unknown` decides no structure** (`E0151`) and `analog` refuses a value that may be unknown
   with `E0150` (§3.3.5, §3.3.8).

It supersedes nothing and extends MD-62 (the one device contract) and MD-64 (the IR's value
forms and unknown-ness as an elaboration analysis).

**Scope:** `crates/piperine-lang` (`elab/`, `types/`), `crates/piperine-ir`,
`crates/piperine-plan`, `crates/piperine-transpile` (`rt/`, `print/`), `crates/piperine-abi`,
`crates/piperine-digital-solver`, `crates/piperine-api`, `crates/piperine-cli`, `docs/spec`.
**Status:** Accepted (2026-11-19), from the approved `.specs/features/unknown-semantics/design.md`.

### MD-71: a contribution stays where it was written; a module is lowered once per value of its key params

`analog-structure` (2026-11-20). The analog half of the language runs the forms Part IV §11 and
Part III §8.3 are built on. The note on the number: `host-ii` and `solver-hygiene` wrote MD-68 and
`unknown-semantics` holds MD-70, so this one is MD-71; the second feature to merge renumbers.

1. **A contribution under a selector stays in place in the IR.** The arms of an `if` or a `match`
   keep their contributions; a branch's kind is decided once every contribution is lowered: a
   branch whose potential contribution depends on a carrier, the environment or a value input is
   a *switch branch* (one private flow pin, one selected equation, `E0418` from the device when two
   selectors hold), and a selector that reads a probe of a potential-capable branch is `E0414`.
2. **A `ddt` that is not a whole addend of an unconditional contribution is a derivative site** on
   its own private pin (`d<k>.rate`, `d<k>.state`, abi.md A1's rule), so `2.0 * ddt(q)`, a `ddt` in
   a `let`, an event read and one with an `abstol` share one mechanism. A core module's inputs
   (`limit`, `ac_stim`, `bound_step`) are evaluated standalone, outside every condition.
3. **A module is lowered once per value of its key params.** A param read by a params-only condition
   that encloses a contribution, by an initial value, or through `inst.name` is structural: the
   body is a variant named `Name~p=v`, a `--param` of such a param selects the variant before the
   run, and a host `set` of it is refused (`E0803`). A zero-potential branch no flow reads merges.
4. **A module with a value input, a result or an `analog` block is hoisted** like a module with a
   result (Appendix C.1.1): the caller's expression is read where the callee reads it.
5. **A parent reaches a child through the IR** (`inst.name`): a reached node or carrier is a node or
   carrier of the parent's body bound by placement to the child's own; `p.inflow()` is an ammeter, a
   zero-potential branch on an inside node; the authored `Child` names its own nodes with
   `Terminal::Own`. A param's start value and a before-run binding are no change: `changed()` is
   false and `prev()` is the value until the first timed `set` (the one cross-lane touch, in the digital
   store).

It supersedes nothing and extends MD-64 (hoisting) and MD-69 (derived params).

**Scope:** `crates/piperine-ir`, `crates/piperine-plan`, `crates/piperine-transpile`,
`crates/piperine-lang` (`types/`, `elab/`), `crates/piperine-analog-solver` (the `CAP` watch),
`crates/piperine-digital-solver`, `crates/piperine-solver`, `crates/piperine-api`.
**Status:** Accepted (2026-11-18), from the approved `.specs/features/analog-structure/design.md`.

### MD-72: the root package holds the integrated tests

`root-e2e` (2026-11-19). The Book and the ngspice scenarios are held at the root.

1. **The root `tests/` holds the integrated tests.** `tests/book/` has one target per Book chapter
   (`book_<chapter>`, declared as `[[test]]` in the root manifest), run through `piperine::` for every
   model, claim and scenario. `tests/book/inventory.tsv` is the registry of every Book block and every
   value sentence, each with its verdict (`run`, `refused`, `holds`, `binary`, `prose`, `shown`,
   `fragment`, `unraised`) and the test that holds it. Two guards (`inventory_guard.rs`,
   `claims_guard.rs`) hold it to the Book: a Book edit updates its rows, and no row is `pending`,
   `diverges` or `disputed`.
2. **`tests/e2e/` is the one home of the ngspice scenarios.** Each twin is `tests/e2e/twins/<name>.{msl,cir,command,stdout}`
   (plus `devices.msl`); `tests/scenarios.rs` runs each through `piperine::` against ngspice under the
   single `ToleranceContract`, and `piperine-parity` stays the harness. The binary's own output stays held
   in `piperine-cli`, where cargo builds it.
3. **A Book claim the binary does not meet is classified by cause**: fixed in the code, `refused` or
   `unraised` under an owner feature in `GAPS.md` section 11, or, for a sentence the code rightly
   contradicts, corrected in the Book (the buck start-up, user 2026-11-19: the first tick acts inside the
   operating point, as part V section 16 step 4 says).

It amends MD-28's "root `tests/` holds no Rust target" and extends MD-31 and MD-65.

**Scope:** `tests/book/`, `tests/scenarios.rs`, `tests/e2e/`, `crates/piperine-parity`,
`crates/piperine-cli/tests/book.rs`, `docs/book`, `GAPS.md` section 11.
**Status:** Accepted (2026-11-19), from the approved `.specs/features/root-e2e/design.md`.
**Number:** `MD-70` is `unknown-semantics`, `MD-71` is `analog-structure`; `MD-72` is the next free one after them.

### MD-73: a host scenario of an example is a package under `tests/e2e/<example>/`

`buck-scenarios` (2026-11-20, proposed while the user was away; renumber at merge if another lane took 73).

1. **The scenario package.** `tests/e2e/<example>/` holds a `Piperine.toml` with a path dependency on the
   example and a crate root whose top exposes the example's `in` ports as its own. Each `[run.<family>]` names
   what is sampled (`print`, `print_step`) and emitted (`streams`); the example's own manifest is not touched.
2. **The stimulus is the Rust host's** (`Session::drive`, `Session::set_at`), one mechanism for every scenario,
   because a timed `set` is not expressible in a bench (X8).
3. **The harness types live in `piperine-parity::scenario`** (`Stimulus`, `Trace`, `DiscreteTrace`,
   `ScenarioRun`, `ScenarioBatch`, `CheckTally`), each with inline unit tests.
4. **One root target per example** (`tests/<example>_scenarios.rs`) runs every family once, all at once, and
   stores each result so a failure is reported, never re-run.

**Scope:** `tests/e2e/buck/`, `tests/buck_scenarios.rs`, `crates/piperine-parity/src/scenario/`.
**Status:** Proposed (2026-11-20), from `.specs/features/buck-scenarios/design.md`.

### MD-74: `settle` runs as section 15 says: held settle commits landed by the owner; an event read as a `bool` is `fired(k)`

`unknown-semantics-events` (2026-11-20, written at T23 while the user was away; completed at T31). The
number: `unknown-semantics` holds MD-70, `analog-structure` MD-71, `root-e2e` MD-72 and `buck-scenarios`
MD-73 (which `composite-values` had reserved), so this is the next free one, MD-74; the second feature to
merge renumbers.

1. **`settle` is an event with a pin of its own.** It fires once per time point, after the instant has
   gone quiet (section 13 rule 17, section 15 step 4); the plan appends a `SETTLE` pin after the
   derivative abstol pins, so no existing shape moves. A commit made in the settle phase is held by the
   digital solver and landed by the owner at the next instant (S1 to S7); a commit to a record pin in
   that phase is no settle commit and applies as the instant exits.
2. **An event read as a `bool` is `fired(k)`.** Outside an event position, in a guard or a reaction body,
   an event expression types `bool` and lowers to `fired(j)` of a body event `j` (section 13 rules 2 and
   9); an event read where another type is expected is `E0441`.
3. **`limexp` runs from the standard library.** A closure's parameters are bindings the module item walk
   skips (they are typed with their closure), so `limexp` checks and lowers like any module with a
   result; it is one `limit` instance per use, its argument moves by at most 1.0 per iteration and its
   derivative is `limexp` (section 22.2.4 rules 3 to 5).
4. **Parity measured, never tuned.** The `limexp` diode agrees with `diode_series.cir` under `DC_VOLTAGE`,
   and the Book's clipper (`tran_clipper.cir`, ngspice given the harness's own step cap) agrees under
   `TRAN_VOLTAGE`; no tolerance moved. The Book's buck holds its `regulated` check from 10 ms on (largest
   deviation 9.4 mV against 50 mV).

**Scope:** `crates/piperine-{ir,plan,transpile,digital-solver,solver,lang}`, the Book's chapters 8 and 10.
**Status:** Batches B1 to B4 delivered (T1 to T23); B5 (event functions, L51 and L53) waits for
`composite-values` and `generics`.

### MD-75: params of every carried type; the host drives a top's node through an IR source; files read at elaboration; every spec code raised or owned

`params-and-io` (2026-11-20, batches B1 to B4; B5 and B6 wait on `composite-values` phases A to B). The
number follows MD-74 (`unknown-semantics-events`, merged first).

1. **Every code of the spec is raised or owned.** `crates/piperine-lang/tests/spec_codes.rs` lists every
   code of every error table under `docs/spec/`, and fails for one that no non-test source spells and
   no row of `tests/support/code_owners.tsv` (owner feature and construct) holds, and for a row whose
   code a source now raises; a type mismatch is `E0180`, no source spells `P0002`.
2. **The host drives a top's node ports** with one `HostSource` body per nature (a potential source to
   `gnd`: DC, AC stimulus, piecewise-linear); a run with a node port and no source is `E0810`, a top with
   a value input is `E0811` (spec §8.1.14, §23.1). ngspice parity runs under the existing contracts.
3. **A param of module type** is a key param holding its template; a placement through it places the
   template's module with the template's constants.
4. **Files are read at elaboration.** `include_str!` is a `const` expression in a `const` item only
   (`E0610`); `IncludedFiles` resolves the path beside the source file, reads each path once
   (`E0612`, `E0613`), and a change to the file reaches nothing until the next elaboration (§20).
5. **The shipped sources are in the formatter's form:** the stdlib, `examples/buck` and the gallery
   fences, with `piperine fmt --check` guarding them; Appendix C.2 states the `table_model` spline end
   conditions.

**Scope:** `crates/piperine-lang` (`types/`, `elab/`), `crates/piperine-api`, `crates/piperine-cli`,
`crates/piperine-parity`, `docs/spec`, `GAPS.md`.
**Status:** Accepted (2026-11-20), from the approved `.specs/features/params-and-io/design.md`.

### MD-76: pure value semantics: no reference, no move, no mutability marker

`value-semantics` (2026-11-21; the number follows MD-75, `params-and-io`; user decisions D1 to D8 of `.specs/features/value-semantics/spec.md`; the numbers V1 to V7 are Appendix G.71's).

1. **Every value is copied where it is used** (§1.4.6); hardware is reached by identity and changes only by a definition, a commit or a contribution (§1.4.7). There is no reference, no borrow, no slice, no lifetime, no move.
2. **Every local is a place** (§1.5.12): `let x = e;` may be assigned. A parameter, a closure parameter, a captured name, `self` and a body `let` are not (`E0183`). `mut`, `ref` and `move` are reserved words (`E0903`).
3. **`Clone` and `Copy` are not names of the language**: no row of the built-in table, `E0161` in a derive, a bound or as `clone()`, with the help "every value is copied; delete the name". The Rust the backend writes still derives them for every generated type: every MSL value is plain data.
4. **`str` is a value type; a range index with const bounds is an array** (`[T; b - a]`); `resolve` takes `[T; N]` under `const N`.
5. **The IR has no slice** (`Expr::SliceOf`, `ValueType::Slice` deleted); `Local::mutable` says the body assigns the local.
6. **Defaults taken while the user was away:** the parser reports a removed form and reads on as the plain form (one use, one diagnostic); a slice type `[T]` is E0900 at the type; the lifetime after a `&` is one `E0905` and no token; the per-task Full gate was replaced by one full root run at the end of the batch (user, 2026-11-21).

**Scope:** `crates/piperine-lang`, `piperine-ir`, `piperine-interp`, `piperine-transpile`, `std`, `examples`, the Book, `GAPS.md` S7 (closed).
**Status:** Delivered (2026-11-21).

### MD-77: the buck runs on demand; the integrated suite is the gate

`integrated-tests` (2026-11-21). The number follows MD-75 (`params-and-io`); MD-76 is `value-semantics`'s
(on `feature/composite-values`, not yet on `main`) and is left to it, so the two never collide at merge.
The feature's spec and design call this decision MD-76 (D16) and are renumbered here.

1. **A target that simulates the buck is excluded from `cargo test` by `test = false`** (D1), in a
   `[[test]]` table of its own crate's manifest: `buck_scenarios` and `book_buck` (root), `buck_whole`,
   `buck_dpwm`, `buck_control`, `buck_boundary`, `buck_power`, `buck_supervisor`, `buck_closed_loop`
   (`piperine-parity`), `buck_long` (`piperine-api`), `buck` and `book_buck` (`piperine-cli`). Paths, test
   names and every historical `**Run**` line stay valid: `cargo test -p <crate> --test <name>` selects a
   `test = false` target. No `#[ignore]`, no `cfg`, no feature.
2. **`test = false` cannot hide a test** (D2): `ON_DEMAND_TARGETS` in
   `crates/piperine-lang/tests/suite_hygiene.rs` registers every flagged target; the guard fails for an
   unregistered one, for a registered one that is gone or runs by default, and for one that
   `examples/buck/README.md`'s "On demand" block does not name as `--test <name>`.
3. **When the buck runs** (D11): once at a feature's close, before its merge, for a feature that changes
   the runtime, the printer, the plan, the solvers or the lowering of an existing form, and in every task
   that edits `examples/buck/**`, `tests/e2e/buck/**` or a buck target. Per-task and per-batch gates use the
   integrated suite. A feature that adds a `test = false` target adds its registry row and README line, and
   its close compiles the on-demand targets (`--no-run`), since `--all-targets` no longer does.
4. **The mapping of historical commands.** `buck_commits`, `buck_records` and `buck_time` are now
   `source_commits`, `source_records` and `source_time` (language proofs that stay in the default run,
   fixtures under `crates/piperine-parity/tests/digital/`); `the_buck_block_is_what_the_binary_prints` runs
   under `cargo test -p piperine-cli --test book_buck` (it was under `--test book`); the 2 us run of
   `cli#44` is `the_buck_bench_starts_with_the_supervisor_off` under `cargo test -p piperine --test
   book_buck`. Closed features are not re-proven.
5. **The suite** (`tests/integrated/`, package `tests/e2e/integrated/`): `threshold`, `sample_hold`,
   `pwm_filter`, `state_machine`, `pi_loop`, run at once by one `ScenarioBatch`; the analog half is
   cross-checked against ngspice by `tran_square_rc.cir` under the existing `TRAN_VOLTAGE`.

**Gate migration** (spec "Gate migration", D12):

The replacement lines each in-flight feature applies to its `tasks.md` at its next rebase onto `main` (D12).
"Integrated targets" is the line `cargo test -p piperine --test integrated; cargo test -p piperine-cli --test
integrated; cargo test -p piperine-parity --test source_commits --test source_records --test source_time`.
"The buck run" is the README's on-demand block.

| Feature (branch) | Lines that cite the buck | Replacement |
| ---------------- | ------------------------ | ----------- |
| every one below | the header line `Buck targets: cargo test -p piperine-parity --test buck_whole ... --test buck_power` | `Integrated targets: <the line above>` and `The buck run (on demand): examples/buck/README.md, "On demand"` |
| every one below | the "Parity is untouchable" sentence "Every buck target passes after every task that touches ..." | "Every integrated target passes after every task that touches ...; the buck run passes once at the feature's close, before merge" |
| every one below | the `Full` gate row "Quick, then the buck targets, `cargo test -p piperine-cli --test buck --test twins [--test commands]`, `cargo test -p piperine-api --test buck_long`, ..." | "Quick, then the integrated targets, `cargo test -p piperine-cli --test twins [--test commands]`, ..." (the rest of the row as it is) |
| `composite-values` | T26 (line 1192) "Every buck target passes with unchanged assertions"; B8 note on T48 | "Every integrated target passes with unchanged assertions"; T48's note stays (`elab_buck`'s pin is a default static proof) |
| `resolved-nets` | lines 36, 47, 89 | the three common rows |
| `unknown-semantics-per-level` | T8 (line 334) "Every buck target passes with only the listed assertions moved"; B2 note | T8 edits assertions inside buck targets, so its gate is the integrated targets **and** the buck targets it edits, run by name (D11) |
| `analog-operators` | line 359 (done) "every `buck_*` target ... pass unedited" | done, historical; B4 to B7 gate with the integrated targets |
| `generics` | T32 (line 840) "Every buck target passes"; B5 note | "Every integrated target passes"; the buck run at the feature's close |
| `params-and-io` B5, B6 | T26 (line 777: it edits `examples/buck/src`), line 846 (rebase), line 913 (byte-identical) | T26 keeps the buck run (D11: it edits the example); line 846: "the `**Build**` line and the integrated targets pass at the rebased `HEAD`"; line 913: "the transpile goldens are byte-identical and every integrated target passes" |
| `unknown-semantics-events` B5 | lines 37, 48, 88 (B5 = T24 to T31) | the three common rows; T5's done-when citing `the_buck_block_is_what_the_binary_prints` under `clibook` is done and historical (the test now runs under `--test book_buck`) |


**Scope:** `Cargo.toml` manifests, `crates/piperine-lang/tests/suite_hygiene.rs`, `crates/piperine-parity`,
`crates/piperine-cli`, `tests/`, `examples/buck/README.md`, `CLAUDE.md`, `AGENTS.md`.
**Status:** Accepted (2026-11-21), from the approved `.specs/features/integrated-tests/design.md`.

---

### MD-79: composite values are typed IR aggregates; every enum is an IR enum; known control prints plain Rust

`composite-values` phases A and B (2026-11-21; the number follows MD-77, `integrated-tests`; MD-78 is
`explicit-unknowns`'s, approved and not yet merged, and is left to it). It extends MD-64 (the IR) and MD-67
(the pair in value code) and supersedes nothing.

1. **The IR holds aggregates as types.** `ValueType::{Tuple, Array, Struct, Enum, Option}`, a type arena of
   `TypeData` (name, MSL path, origin, fields or variants, derives) and `Constant::Aggregate` (an entry of an
   arena of composite constants, so `Constant` stays `Copy`). Every enum is an IR enum, a unit one included; its
   carried bytes are unchanged (the tag), and its variant names live in the arena, never in a net.
2. **Control.** A known decision prints plain Rust; an unknown one prints `rt::Unrolled` and the merged tuple of
   the locals it wrote (§3.3.3). Loops and recursion that place or contribute are unrolled at elaboration.
3. **The ABI is unchanged.** A composite carrier, param or record field is one pin laid out by
   `piperine_plan::CompositeLayout`: the parts' value bytes one after another, the first lowest and unpadded
   (what `rt::flag::Carried` writes), then one known byte for the whole value (one flag per value, G6/S6); one
   ABI field per leaf named by its dotted path, each holding that known byte; an array one field per element.
   The layout is a function of the IR type alone (abi.md P4).
4. **The host (Q2 (c), D13).** `piperine-api` decodes a composite pin through the arena (`CellShape`,
   `CompositeCell`); the text table, record lines and JSON Lines write literal syntax (JSON nested), CSV and the
   plot one column per dotted leaf, a printed path naming a leaf prints that leaf alone, and an enum with no
   payload reads as its variant's name (a record field `Field::Variant`, a table cell `Cell::Text`). A composite
   param is set by the path of one leaf before the run (`Top.f.num[2]=0.5`); its `where` is checked on the whole
   value after the write (`E0317`).
5. **Defaults taken while the user was away** (D1 to D20 of `design.md`, plus B1 to B10 of `tasks.md`'s Handoff).

**Scope:** `crates/piperine-ir`, `piperine-plan`, `piperine-transpile`, `piperine-lang`, `piperine-api`,
`piperine-cli`, `piperine-parity`, `docs/spec`, the Book, `GAPS.md`.
**Status:** phases A and B delivered (2026-11-21); phases C and D (arrays of carriers, `f16`/`f128`) follow.

## Handoff Snapshot

### Feature - `solver-abi-integration` (2026-11-15)

The Element contract and the two-phase digital engine are gone (MD-62); every
analysis runs on devices over the owner protocol. Tasks T1 to T25 are
committed; the Verifier and `check_proofs.py` are the orchestrator's next step.
State: see `.specs/features/solver-abi-integration/tasks.md` `## Handoff`.

### Feature - `solver-restructure` (2026-11-14)

Structure-only split of the solver into three crates (MD-60), five crates and
the VAMS end-to-end surface removed, hygiene guard moved into `piperine-lang`.
State: see `.specs/features/solver-restructure/tasks.md` `## Handoff`.

### Feature — `lrm-conformance-wave3` — CLOSED (2026-08-30)

All 55 tasks (T1-T55, Phases 1-8) executed and committed on
`feature/device-verification`. Verifier PASS (with 3 gaps found and
fixed post-hoc, re-verified independently) —
`.specs/features/lrm-conformance-wave3/validation.md`. Both coverage
gates now green: axis-A (`tests/lrm_section_coverage.rs`) reaches zero
uncovered LRM sections (612 headings: 588 Covered, 24 Excluded), the
project's long-standing known-red baseline target; axis-B
(`tests/spec_section_coverage.rs`), built from scratch this wave, reaches
zero unverdicted `docs/spec/` headings (284: 15 Covered, 269
DeclaredLimitation). `cargo build --workspace` zero warnings,
`cargo test --workspace` fully green, no known-red targets left. Notable
follow-ups disclosed but deliberately left open (see validation.md and
individual commit bodies): §4.5.15's `if`/`case`/`?:` conditional
restriction not enforced (real SPICE headers use the pattern); `wire`/`reg`
misclassifying as analog in the continuous-assign domain check
(`elab/lower/vams.rs`); most of axis-B's 269 headings remain
`DeclaredLimitation` pending further backfill in a future wave, same
multi-wave pattern axis-A itself followed.

### Feature — `lrm-conformance-wave3` (SPEC + DESIGN + TASKS 2026-08-27, execution not started) — historical pre-execution research below

**Why it exists.** Wave 2's own closure gate was never green:
`tests/lrm_section_coverage.rs::covered_and_excluded_partition_the_lrm` fails
with **105 of 612 LRM-2-4 sections carrying no verdict** (475 Covered, 20
excluded). Wave 2's tasks.md predicted T69 would close the matrix; T69 closed
364 of 469 and the remainder was never re-planned.

**What direct probing found** (one minimal fixture per suspect clause,
elaborated/compiled against HEAD of `feature/device-verification`):

- **Vectors/buses do not exist.** No range form parses anywhere —
  `electrical [0:3] bus`, `input [0:3] p`, `reg [3:0] q`, `wire [3:0] w`,
  part-select `b[7:4]`, memory `reg [3:0] mem [0:7]`. The post-declarator
  form (`electrical bus[0:3]`) parses but does not lower a literal index.
  `README.md`'s advertised `generate` ladder over `electrical tap[n-1:0]`
  could not compile.
- **Two parse-and-vanish constructs** (fail-loud violations of the same class
  as wave 2's §1): `specify … endspecify` (body stored as an unread `String`)
  and `config … endconfig` (`ConfigDecl { body: String::new() }`). `specparam`
  and in-module `task` declarations are also no-op match arms.
- **Illegal-but-accepted:** an analog operator under a non-constant conditional
  (§4.5.15/§5.8.4) and a NaN/±infinity contribution (§7.3.2.1).
- **Missing families:** port branches `branch (<p>)` (§3.12.1), nature-attribute
  access `a.potential.abstol` (§5.5.3), hierarchical contributions (§5.6.8),
  paramset output variables (§6.4.3), standard attributes `desc`/`units`/`op`/
  `multiplicity` (§2.9.2), discipline attribute override + nature-from-discipline
  derivation (§3.6.2.5/6/7), out-of-module discipline declaration (§3.10), drive
  strengths (A.2.2.2), PLA tasks (§9.8).

**Also fixed in this session (unrelated red tests, not part of the wave):**
`piperine-solver`'s `digital::events` region-race tests asserted a *region*
race using two distinct `source` ids, which T54's real `DriverTable` correctly
resolves to `X` per Table 7-3; rewritten (never deleted) to use one source, with
a new `two_distinct_drivers_contend_per_table_7_3` pinning the other half.
`vams_port_connected::analog_node_alias_fails_loud_citing_md18` was stale after
T56 gave the constant form a resolution path; rewritten to pin the disclosed
dotted-hierarchical-constant behavior (folds to `0`).

**Docs.** `docs/spec/` was rewritten this session as the authoritative technical
specification (index, Part I, Part II, the new Part III "LRM Conformance"
replacing the retired Part III stub and absorbing the root `SPEC_DEVIATION.md`,
Part IV, Part V, Part IX retitled to `piperine-ir`, Part VII extended with
§21–§23 pole-zero/distortion/S-parameters, cross-crate renames elsewhere).
`README.md` was rewritten in a "what you can do" shape with an honest
"What doesn't work yet" section. Root `SPEC_DEVIATION.md` moved to
`.specs/features/lrm-conformance-wave2/`.

**Two axes (superset rule).** The spec was widened after review: axis A is
LRM↔impl (the 105 sections), axis B is **Piperine's own spec↔impl** — the
superset surface the LRM never mentions (`constraint`/`tol`/`const`/doc
comments/`@device`/`rfport`/UIC seed, the whole host API incl. the
optimize-center-high-sigma driver, sampling, Monte Carlo, aging, coverage, the
host device library and sequencer, plugins, POM+selector, `piperine-ir`, CLI).
Axis B has **no coverage gate at all today**; building one is `PSW3-10`. The
superset rule is normative in the spec's §0: a deliberately unimplemented LRM
clause is an exclusion naming its replacement (never a defect), a deliberate
difference is a pinned deviation, and an extension is not exempt from proof.
Axis-B probed gaps B1–B10 (inert selector axes, unreachable POM/error variants,
Rust-only host surfaces, stale `piperine-ir` ownership claim, untested
`` `include`` legacy aliases, a self-describing-as-expected red gate, no
extension gate, retired-name example) carry ids `PSW3-01..10`.

**Design + tasks written (2026-08-27).** `design.md` corrected the spec's
biggest assumption: **most vector machinery already exists** (`NetType::Array`,
per-element node registration `bus[0]..bus[N-1]`, `NetRef{net,index}` lifting,
`Expr::Index`/`PartSelect` in AST *and* grammar). The real seams are three:
the leading `[range]` in the grammar, `ident_from_expr`'s `Expr::Ident`-only
terminal resolution (`resolve/pom/expr.rs:54`), and the single `unsupported`
arm for digital index/part-select (`emit/digital_expr.rs:79`). Two other spec
assumptions were corrected: `parse_specify_block` skips tokens and returns an
empty item list (so the grammar must be *written*, not just lowered), and
NaN/inf contribution is already detected — as a generic convergence failure,
so the work is retargeting the message.

Confirmed decisions: **MD-52** (digital vectors scalarized per bit) and
**MD-53** (axis-B `SPEC-COVERS` gate mirroring MD-51).

`tasks.md`: **55 tasks in 8 phases** (1 vector foundation · 2 vector access ·
3 specify+config · 4 discipline/nature/attributes · 5 branches/contributions/
restrictions · 6 axis-A coverage closure · 7 axis-B gate · 8 axis-B gap
closure), with a Test Coverage Matrix derived from CLAUDE.md/AGENTS.md/MD-28/
MD-31/MD-51 + suite_hygiene, gate commands, and all three pre-approval checks
(granularity, diagram cross-check, test co-location) green.

**Next step:** user approval of tasks.md, then Execute (8 batches at the
~7-task budget — sub-agent offer pending).


**Last updated:** 2026-08-24 — `lrm-conformance-wave2` DELIVERED WITH
DISCLOSED RESIDUE (T1–T72, all 72 tasks committed, branch
`feature/device-verification`); prior entries below (ngspice-parity,
codegen-simplification, dv-tester, dv-gradients, etc.) predate this and are
kept as history.

### Feature — `lrm-conformance-wave2` (DELIVERED WITH DISCLOSED RESIDUE 2026-08-24)

Spec/design/tasks/baseline in `.specs/features/lrm-conformance-wave2/`
(`spec.md`/`design.md`/`tasks.md`/`baseline.md`, none committed to git —
pre-existing untracked state on this branch, not produced or fixed by this
closure batch; `baseline.md` is tracked and was updated). All 72 tasks
(T1–T72) are committed on `feature/device-verification`; T69–T72 (this
final batch) landed as commits `1a335387` (T69), `90cf5192` (T70),
`b3ee793b` (T71) — T72 is this STATE.md update.

**What T1–T68 delivered (prior batches, already on this branch before this
session):** re-verified the entire pre-wave-2 `SPEC_DEVIATION.md` audit
(63 findings across §1 drops-silenciosos, §2 semantic divergences, §3
missing constructs, §4 ignored operator args, §5 disclosed deviations) and
closed 51 of them with dedicated tasks/tests/commits, plus built the
`tests/lrm_section_coverage.rs` mechanism (MD-51) that derives LRM-2-4
section coverage from `//! LRM-COVERS:` test-header declarations rather
than a hand-maintained registry.

**What this batch (T69–T72) delivered:**

- **T69** (`1a335387`) — drove `covered_and_excluded_partition_the_lrm`
  from 469/612 uncovered sections down to **105/612 uncovered**, via ~90
  backfilled `LRM-COVERS` annotations (cross-checked against real test
  bodies, not filename-guessed), a handful of new minimal tests, and 12
  Annex E ids + 2 Annex C ids added to `coverage_exclusions.toml`
  (`out_of_scope`/`informative`, each mapped to a real spec.md row or LRM
  front-matter tag). **The gate stays red (105 uncovered)** — every one of
  those 105 was individually source-verified to describe LRM functionality
  Piperine genuinely does not implement yet (PLA tasks, stochastic
  `$q_*`, driver-access functions, hierarchical branch contributions,
  paramset output variables, the Annex-C Verilog-A-subset compliance mode,
  ~20 disclosed-unimplemented VPI routine categories, and more). None map
  to spec.md's Out of Scope table, so none could be excluded; closing them
  for real means implementing the feature, not writing a test. **AC20's
  letter ("zero empty cells") is not met; its spirit ("no claim without an
  assertion that exercises the clause") is upheld throughout** — nothing
  was fabricated to force a green run.
- **T70** (`90cf5192`) — regenerated `SPEC_DEVIATION.md` against HEAD: of
  the pre-wave-2 audit's 63 claims, 51 are now closed (each citing the real
  commit hash that fixed it, verified via `git log`/`git show`, plus
  current `file:line` evidence the bug/gap is gone) and 9 stay open as
  disclosed rows with re-verified evidence and explicit LRM clauses — all
  documented scope cuts (fail loud, not silent), not forgotten bugs.
- **T71** (`b3ee793b`) — final per-target A/B appended to `baseline.md`
  (original snapshot untouched): 317 of 318 baseline targets
  equal-or-better with zero failures; the one exception
  (`vams_port_connected`'s `analog_node_alias_fails_loud_citing_md18`) is a
  pre-existing red that predates this batch, not introduced by T69–T72; 80
  new targets landed across wave-2, all green except
  `lrm_section_coverage`'s own known partition gap. `cargo build
  --workspace` stays 0 warnings throughout. Also appended a second,
  independent `WAVE2_REGISTRY` to `tests/lrm_conformance_matrix.rs` (pure
  append, wave-1's `REGISTRY` untouched — 235 insertions, 0 deletions)
  covering all 66 `LRMW2-*` requirement ids: 64 `Closed` naming a real
  test, 2 (`LRMW2-01` baseline capture, `LRMW2-66` SPEC_DEVIATION.md
  regen) are process/doc tasks with no assertion to name, disclosed as
  `Pending` rather than fabricated and locked to exactly those two ids by
  a dedicated guard test.
- **T72** (this commit) — this STATE.md update.

**Known non-regressions carried into the next session (none of these are
new — all predate T69–T72, confirmed unchanged by T71's A/B):**
- `covered_and_excluded_partition_the_lrm` — 105/612 LRM-2-4 sections
  genuinely uncovered (see T69 above); closing further requires
  implementing real features, not more test-writing.
- `vams_port_connected.rs::analog_node_alias_fails_loud_citing_md18` — one
  pre-existing failing test, root cause not investigated by this batch.
- Two `piperine-solver` inline unit tests under `src/digital/events.rs`
  (`inactive_region_lands_after_active_at_the_same_timestamp`,
  `stratified_regions_resolve_the_canonical_active_nba_race_through_
  evaluate_until_stable`) — root-caused by a prior batch to
  `apply_driven_event` universally strength-arbitrating same-net events; a
  targeted fix needs net-kind classification `DriverTable` doesn't have
  yet. Deliberately left red and re-disclosed, not silently carried.

**Decisions:** MD-51 (conformance declared at the test, gate derives the
matrix) is unchanged and was followed throughout this batch — nothing in
T69–T72 contradicts it; the 105-section residual is exactly the kind of
disclosed gap MD-51's own design anticipated (a stale claim fails loud
instead of drifting silently), not a mechanism failure. No new MD entry
was added — the residual-gap disclosure lives in the T69 commit body,
`SPEC_DEVIATION.md`, and `baseline.md`'s appendix, not as a fresh
standing decision.

**Next step for a future session (if resumed):** either (a) accept
`lrm-conformance-wave2` as delivered-with-disclosed-residue and start a
follow-on wave scoped explicitly to implement the 105 remaining LRM
clauses (each already has a `file:line` pointer to where the gap lives,
via `SPEC_DEVIATION.md` §6 "Ainda aberto" and the T69 commit body's
category list), or (b) route each 105-item gap into ROADMAP.md as
individually trackable follow-up items. This session did not choose
between them — that is a product-priority call, not an implementation
one.

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

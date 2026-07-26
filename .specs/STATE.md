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

**Status:** Locked (user, 2026-07-20 — `codegen-architecture` feature,
`hierarchy-flattening`-adjacent readability pass). Applies to future codegen
modules: name by pipeline stage, narrow visibility by evidence not habit.

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

---

## Handoff Snapshot

**Last updated:** 2026-07-26 — `p6-cleanup-completeness` hygiene subset
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

### Feature — `solver-simplification` (IN PROGRESS — batch 6 remaining)

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

**Baseline at batch-5 close:** `cargo test --workspace` 520 green /
5 ignored, 0 rustc warnings; parity baselines bit-identical through every
batch.
**Remaining:** batch 6 (P9) — Part VII canonical rewrite (T33–T35), then
the feature Verifier.
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

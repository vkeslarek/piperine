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
`ngspice-parity` specification).

---

## Handoff Snapshot

**Last updated:** 2026-08-04 — `ngspice-parity` IN PROGRESS (Phases 1–2 done,
T1–T10 committed); `codegen-simplification` DELIVERED 2026-08-03 (T1–T14,
Verifier PASS with 2 minor non-blocking gaps); `dv-tester` DELIVERED
2026-08-01; `dv-gradients` DELIVERED 2026-07-31.

### Feature — `ngspice-parity` (IN PROGRESS 2026-08-04)

Spec/design/tasks in `.specs/features/ngspice-parity/`. Executes MD-38 (parity
measured, not asserted): extend the golden oracle to AC, noise, transient and
temperature, then close the deviations it measures across the 13 shipped
models.

**Done — Phase 1 (oracle foundation) and Phase 2 (domain comparators + gate),
T1–T10.** `tests/support/ngspice.rs` now holds the shared `NgspiceHarness`, the
N-column `WrData` parser and the seven-domain `ToleranceContract` (NGP-07: the
only place a numeric tolerance may be written). Five per-domain golden targets
(`ngspice_{validation,ac,noise,tran,temp}.rs`) sit on it, and
`tests/ngspice_parity_matrix.rs` is the MD-31 coverage gate — 13 models × 6
domains, each cell `Covered { fixture }` or `NotApplicable { reason }`, all four
asserts proven able to fail. Root-target naming gained its first registered
exemption in `suite_hygiene.rs` (the matrix links no workspace crate), with the
assertion turned into an equality so the exemption cannot grow quietly.

**Remaining:** Phase 3 (T11–T18, model fixes), Phase 4 (T19–T20, `urc` taper +
`ISPERL`), Phase 5 (T21–T23, `gap-3` array-net expansion + generic `urc[N]`),
Phase 6 (T24, ROADMAP rewrite), then the Verifier.

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

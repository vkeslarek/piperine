# Piperine — Claude Code Instructions

## Project summary

Piperine is a mixed-signal language and simulator. `piperine-lang` parses the Piperine
mixed-signal language (MSL, specified in `docs/spec/`) and builds the POM `Design` from it;
elaboration and code generation are not built yet. The native solver is three crates:
`piperine-solver` (the analysis engine and the `Element` ABI) and two sub-solvers,
`piperine-analog-solver` and `piperine-digital-solver`, which the solver owner combines.
Devices speak one ABI, the leaf crate `piperine-abi` (MD-61): the `#[repr(C)]` contract of
`.specs/features/device-abi/abi.md`, the `Device` trait, the solver-side `caller::Handle`
and the shape and partials checks; it depends on no crate.
The target is that the analog and the digital side meet only through events (MD-60);
today the digital snapshot (`&[LogicValue]`) is still shared until feature B. Rust workspace, edition 2024.

**Today only `piperine-lang`, `piperine-abi` and the three solver crates compile.** The root `piperine`
shell package, `piperine-project`, `piperine-api`, `piperine-cli` and `piperine-lang-server` are kept in the tree but do not
build yet: they depended on crates that were retired (codegen, ir, python, plugin,
plugin-macros) and wait for the new device compiler. The root `Cargo.toml` lists the five
green crates as `default-members`.

## Pipeline (the spine)

```
source ──parse_and_elaborate──► Design (POM)        crate: piperine-lang   (builds)
                                    │
                                    ▼
                        device compiler (to come)    not in the tree yet
                                    │
                                    ▼
          piperine-solver  ◄── owns ──►  piperine-analog-solver
          (Element ABI, analyses)        piperine-digital-solver
                                    │
                                    ▼
                    hosts: piperine-api (Rust), piperine-cli
```

The owner `piperine-solver` depends on both sub-solvers; the sub-solvers depend on neither
the owner nor each other. `piperine-abi` is the one in-tree crate any of the three may
depend on (MD-61). Events are the target boundary between them (the digital snapshot is shared until feature B).

### UNBREAKABLE RULE — POM navigability mirrors the source

**The POM's navigability reflects the structure of the original code, never the internal
structure of elaboration.** A device author reads back their own modules, instances, and
hierarchy from the POM exactly as written; internal transforms (hierarchy flattening above
all) are codegen concerns they must never leak into `Design::modules`. Every elaboration pass
builds the POM from the immutable AST and only *adds* or *validates* — it never overwrites
authored structure. Monomorphization may *name* a concrete variant (`urc__5`) but must keep
the `instance → submodule → sub-instances` tree walkable. A transform that needs a
collapsed/flattened form (e.g. for codegen) produces a **separate side artifact**
(`Design::flat_modules`, `#[serde(skip)]`), leaving the authored hierarchy intact. See
`.specs/features/hierarchy-flattening/design.md`.

## Build and test

```sh
cargo build                       # the default members: lang, abi + the three solver crates
cargo test                        # their whole suite
cargo test -p piperine-solver     # one crate
cargo test <name>                 # one test
cargo test -- --nocapture         # see solver output
```

At the root, `cargo build` / `cargo test` run the `default-members` (the crates that
compile). Never pass `--workspace`: the root shell and the other four crates do not build yet.

## Crate responsibilities

| Crate | Role |
|-------|------|
| `piperine-lang` | The MSL front end: lexer and parser (`msl/`, logos + chumsky, miette diagnostics, canonical printer) and the Piperine Object Model (`pom/`: an arena `Design` over a crate graph, name and member resolution, reflection, serde), with the embedded standard library (`std/*.msl`). Spec: `docs/spec/`; POM guide: `docs/pom/`. |
| `piperine-abi` | The device ABI (MD-61, `.specs/features/device-abi/abi.md`): the `#[repr(C)]` shape, frames and `u32` constants (`raw.rs`), the shipped layouts `BIT`/`REAL`/`BYTE`/`F64`/`NONE` (`layout.rs`), the owned shape builder `ShapeBuf`/`ShapeRef` (`shape.rs`), the `Device` trait and its `Eval`/`React` lenses (`device.rs`), the generic `extern "C"` thunks `module::<D>`/`library` (`thunk.rs`), the solver-side `caller::Handle`, shape-local `validate` and the finite-difference `verify_partials`. Depends on no crate; `unsafe` only in `raw.rs`, `thunk.rs`, `caller.rs`. |
| `piperine-solver` | The solver owner: DC/AC/transient/noise/TF and the rest of the analysis family (`analyses/`), the `Element` trait + `ElementCapabilities` (`core/element.rs`), `Net` naming, `prelude.rs`. Depends on `piperine-analog-solver` and `piperine-digital-solver`; depends on no other in-tree crate. |
| `piperine-analog-solver` | The analog sub-solver: MNA/linear algebra (`math/`), `Netlist`, `Tolerances`, `ConvergencePlan` (`convergence.rs`) with its homotopy/Newton/stepper strategies, `TrBdf2` + LTE (`math/integration.rs`), `SolverDomain` errors. Depends on no in-tree crate; the events-only boundary to the digital side is the target (the digital snapshot is shared until feature B). |
| `piperine-digital-solver` | The digital sub-solver (event-driven scheduler `scheduler.rs`, drivers, topology, `DigitalLimits`). Depends on no in-tree crate; the events-only boundary to the analog side is the target. |
| `piperine-api` | The library face (MD-20): the one host entry point `Session` + `SessionBuilder`, the sweep drivers, and `SolverConfig` (`session/`), result objects (`results.rs`, `waveform.rs`, `fourier.rs`, `units.rs`), `SimHooks` lifecycle trait (`hooks.rs`), `prelude` re-exports. |
| `piperine-cli` | `piperine` CLI (+ the binary target): `check`, `build`, `run` (python scripts / REPL), `fmt`, `new`, `test` (`*_tb.py` runner), `clean`, `add`, `remove`, `tree`, `plugin`. |
| `piperine-project` | `Piperine.toml` discovery, git dependency resolver (a dependency that declares contributions **is** a plugin — MD-30), `SourceMap` recipe, GitHub-release device-binary fetch + cache (`release.rs`), plugin lockfile. |
| `piperine-lang-server` | LSP server. Handlers share `RequestExt::parse`/`ConnectionExt::respond` (every request id gets a response), `DocumentState::{analyze,resolve_at,word_occurrences}`, `ProjectContext::discover`. |

## Solver architecture (current state)

- **One ABI:** `Element` trait (`core/element.rs`) with `ElementCapabilities` bitflags
  (`ANALOG`, `DIGITAL`, `SAMPLES_ANALOG`, `LOADS_DC/AC/TRAN`, `EMITS_NOISE`,
  `DEPENDS_ON_DIGITAL`, `HAS_INTERNAL_UNKNOWNS`, `SUPPORTS_ROLLBACK`, `BYPASS_OK`).
  No `Device` wrapper, no downcast.
- **Naming:** `Net` (`core/net.rs`) unifies analog nodes, branch currents, digital nets,
  and pseudo variables under one public identity with stable labels.
- **Convergence:** `ConvergencePlan` (analog crate, `convergence.rs`) composes `HomotopyStrategy`
  (gmin stepping, source stepping), `NewtonStrategy`, `StepperStrategy`, and `PlanLimits`
  (caps extracted from magic numbers).
- **Integration:** `TrBdf2` (analog crate, `math/integration.rs`) — the sole transient scheme (a
  Trapezoidal stage over `γh` then a BDF2 stage over `(1−γ)h`, γ = 2−√2); no
  method-selection surface. LTE-driven timestep via
  `Element::suggest_transient_step` + the plan's PI controller.
- **Errors:** `SolverDomain` enum — typed domains, no free strings.
- **Scheduler:** the generic digital scheduler returns `Result<(), DigitalError>`; the owner maps it to `Error` at its one call site.
- **Prelude:** `prelude.rs` exports the host-facing surface.

## Known gaps

- The device compiler, hosts (`piperine-api`, `piperine-cli`), `piperine-project` and the
  language server do not build until the compiler lands; see `ROADMAP.md`.
- Solver ABI work in progress: `.specs/STATE.md` holds the decision log and
  `.specs/features/` the feature specs.

## Naming & conventions

- Ground net → MNA reference; gnd-family names: `gnd/GND/vss/VSS`.
- Elaboration pre-folds `parameter` defaults; `analog function`/`function` default
  arguments are elaboration constants.

## Declared language surface (MD-24, re-spelled for VAMS — see MD-42)

**PHDL-era mechanism, gone (MD-45, not retargeted):** every referenceable name used to
resolve to a textual `extern type`/`fn`/`task`/`operator`/`attribute`/`impl` declaration
in `crates/piperine-lang/headers/*.phdl`. That mechanism has no Verilog-AMS Annex A
production and was deleted in full with `piperine-lang/src/parse/` (T21) — there is no
`extern` keyword in VAMS, and `ElabContext` lost every sub-registry
(`callables`/`types`/`operators`/`constraint_helpers`/`impl_methods`) that backed it.

**What's real today:** primitive types, math/libm functions, system tasks, and runtime
operators are LRM builtins the parser/elaborator recognize natively (Annex A
grammar productions, ch. 4/9 tables) — no textual declaration step, because Verilog-A
never had one either. The **only** surviving declared-surface mechanism is
`@device`/`@port`: `ElabContext::schemas` (T35, VAMS-12/13) is a name-only registry,
seeded before elaboration by `piperine_lang::seed_device_port_schemas`, and consulted by `elab::lower::vams`'s `(* device = ...
*)`/`(* port = ... *)` attribute-instance lowering — an unrecognized schema still fails
loud (`ElabErrorKind::UnknownAttrSchema`), MD-24's "no silent fallback" rule carried over
verbatim even though its declaration vehicle changed. The PHDL-era `extern.phdl` stub mechanism is deleted.

Regression guard (rewritten for VAMS, T36, design R9 "rewritten, never deleted"):
`crates/piperine-lang/tests/extern_coverage_guard.rs`.

## Files not to edit casually

- `crates/piperine-lang/src/{grammar,lexer.rs,preprocessor.rs,parser.rs}` — the
  hand-written, LRM-Annex-A-only recursive-descent Verilog-AMS parser; changes ripple
  through all parsing. Provenance rule (C7): grammar and semantics come from
  `VAMS-LRM-2-4.md` only — never a third-party Verilog-A compiler, and the deleted
  `piperine-ams` is off-limits as a source.
- `headers/`, `tests/fixtures*` — frozen test corpora.

## Where the tests are

A hand-maintained list of test files goes stale silently (P6 found this file
naming a target that had been switched off with `#![cfg(any())]`). So there is
no list here — the tree is the list, and a guard keeps it honest.

- **Enumerate**: `ls crates/*/tests/*.rs`, or `cargo test`
  (each `Running tests/<name>.rs` line is one target). Every target opens with a
  `//!` header saying what it covers — read that first.
- **Placement rule (MD-28)**: a target lives in the crate it exercises; root
  `tests/` holds no Rust target. Integration tests are
  grouped by functionality, one concern per target.
- **The enforcing guard**: `crates/piperine-lang/tests/suite_hygiene.rs` walks the repo's own sources
  and fails on switched-off test code, `#[ignore]`d tests, unregistered
  ```` ```ignore ```` doc fences, a target with no `//!` scope header, file-scope
  lint suppression (MD-33), and dead-architecture identifiers (MD-35).

## Documentation

- Verilog-AMS language reference: `VAMS-LRM-2-4.md` (LRM 2.4, the sole grammar/semantics
  source, C7) — Annex G (change history) and Annex H (glossary) are additionally ported as
  reference material under `docs/reference/` (T57).
- `docs/spec/` (Parts I–VIII + appendices A–G; `index.md` is the map) — the Piperine
  mixed-signal language (MSL) specification: the Rust-like target language for the next
  frontend (11 concepts, 65 constructs beyond Rust). It does **not** describe the current
  Verilog-AMS frontend, whose authority stays `VAMS-LRM-2-4.md`. Design record (adversarial
  critic/builder rounds, review): `.specs/features/msl-spec/`.
- Solver gaps + ABI plan: merged into `ROADMAP.md` (P1/P2)
- Spec-driven feature tracking: `.specs/STATE.md` + `.specs/features/`
- Open items: `ROADMAP.md`

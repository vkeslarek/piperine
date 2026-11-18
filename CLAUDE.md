# Piperine — Claude Code Instructions

## Project summary

Piperine is a mixed-signal language and simulator. `piperine-lang` parses the Piperine
mixed-signal language (MSL, specified in `docs/spec/`), builds the POM `Design` from it,
type-checks the M1 analog subset and the digital and mixed forms of `msl-digital` (`types/`)
and elaborates them into `piperine-ir`'s `Ir` (`elab/`, MD-64). `piperine-transpile` prints the
IR as Rust, builds it with `rustc` into a device library and loads it (MD-63); the frozen
interpreter `piperine-interp` runs the M1 analog subset only. `examples/buck` runs whole from
its `.msl`, closed loop included. The native solver is three crates:
`piperine-solver` (the analysis engine and the owner protocol over the device ABI) and two sub-solvers,
`piperine-analog-solver` and `piperine-digital-solver`, which the solver owner combines.
Devices speak one ABI, the leaf crate `piperine-abi` (MD-61): the `#[repr(C)]` contract of
`.specs/features/device-abi/abi.md`, the `Device` trait, the solver-side `caller::Handle`
and the shape and partials checks; it depends on no crate.
The analog and the digital side meet only through events at an instant, run by the owner
protocol (`piperine-solver/src/protocol/`, MD-60, MD-62): no value or snapshot is shared, and
the device ABI is the only device contract. Rust workspace, edition 2024.

**The crates that compile are the root `Cargo.toml`'s `default-members`:** `piperine-lang`,
`piperine-ir`, `piperine-abi`, the three solver crates, `piperine-interp`, `piperine-plan`,
`piperine-transpile`, `piperine-parity`, and the host crates `piperine-project`, `piperine-api`,
`piperine-cli` and the root `piperine` shell package, which build (feature `msl-cli`). Only
`piperine-lang-server` stays outside the default build: it still targets the retired Verilog-AMS
front end, is not edited, and is deferred by the user to a feature of its own.

## Pipeline (the spine)

```
source ──parse──► Design (POM)                     crate: piperine-lang
                    │  types::Types::check (M1 analog + msl-digital forms)
                    │  elab::Elaborator::run
                    ▼
                  Ir (piperine-ir)                   crate: piperine-ir
                    │  BodyPlan, Netlist::circuit    crate: piperine-plan
                    ▼
                IR → Rust → rustc → device library   crate: piperine-transpile
                (or the frozen M1 interpreter)       crate: piperine-interp
                                    │
                                    ▼
          piperine-solver  ◄── owns ──►  piperine-analog-solver
          (owner protocol, analyses)     piperine-digital-solver
                                    │
                                    ▼
                    hosts: piperine-api (Project, Session) ◄── piperine-cli (`piperine`)
```

The owner `piperine-solver` depends on both sub-solvers and on `piperine-abi`; the sub-solvers
depend on neither the owner nor each other. `piperine-abi` is the one in-tree crate any of the three may
depend on (MD-61). Events at an instant, run by the owner protocol, are the boundary between them.

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
cargo build                       # the default members (the crates that compile)
cargo test                        # their whole suite, ngspice parity included
cargo test -p piperine-solver     # one crate
cargo test <name>                 # one test
cargo test -- --nocapture         # see solver output
```

At the root, `cargo build` / `cargo test` run the `default-members` (the crates that
compile). Never pass `--workspace`: `piperine-lang-server` does not build.

## Crate responsibilities

| Crate | Role |
|-------|------|
| `piperine-lang` | The MSL front end: lexer and parser (`msl/`, logos + chumsky, miette diagnostics, canonical printer `msl::print`, and the formatter `msl::format` behind `piperine fmt`, which keeps every plain comment through `msl::comments`) and the Piperine Object Model (`pom/`: an arena `Design` over a crate graph, name and member resolution, reflection, serde), with the embedded standard library (`std/*.msl`); the type checker (`types/`: `Types::check`, the M1 analog subset and the digital and mixed forms of `msl-digital`, one `P0001` naming each construct outside them) and the elaborator (`elab/`: `Elaborator::run`, `Design` + top module to an `Ir`, reading the POM and never writing it; modules with a result hoisted and unknown-ness computed as `Flagged`, MD-64).  Spec: `docs/spec/`; POM guide: `docs/pom/`. |
| `piperine-ir` | The IR: an elaborated design as an object model (`Ir`: arenas of bodies, functions, instances, nodes, natures behind typed `u32` ids), its builder (`IrBuilder`), `Ir::check()` and the printer (`print_structural`). A leaf crate: no in-tree dependency. Produced by `piperine-lang`'s `elab/`; read by `piperine-plan`, `piperine-transpile` and `piperine-interp`. |
| `piperine-abi` | The device ABI (MD-61, `.specs/features/device-abi/abi.md`): the `#[repr(C)]` shape, frames and `u32` constants (`raw.rs`), the shipped layouts `BIT`/`REAL`/`BYTE`/`F64`/`NONE` (`layout.rs`), the owned shape builder `ShapeBuf`/`ShapeRef` (`shape.rs`), the `Device` trait and its `Eval`/`React` lenses (`device.rs`), the generic `extern "C"` thunks `module::<D>`/`library` (`thunk.rs`), the solver-side `caller::Handle`, shape-local `validate` and the finite-difference `verify_partials`. Depends on no crate; `unsafe` only in `raw.rs`, `thunk.rs`, `caller.rs`. |
| `piperine-solver` | The solver owner: DC/AC/transient/noise/TF and the rest of the analysis family (`analyses/`), the owner protocol (`protocol/`) and its adapters (`analyses/adapters.rs`), the bind of device instances (`core/bind.rs`, `core/builder.rs`), `Net` naming, `prelude.rs`. Depends on `piperine-abi` (the device ABI), `piperine-analog-solver` and `piperine-digital-solver`; depends on no other in-tree crate. |
| `piperine-analog-solver` | The analog sub-solver: MNA/linear algebra (`math/`), `Netlist`, `Tolerances`, `ConvergencePlan` (`convergence.rs`) with its homotopy/Newton/stepper strategies, `TrBdf2` + LTE (`math/integration.rs`), `SolverDomain` errors, the device `Assembly` (`device/`: terms, partials, histories, limit iterates, checkpoints) and the secant step of crossing location (`crossing.rs`). Depends only on `piperine-abi`; it names no digital type. |
| `piperine-digital-solver` | The digital sub-solver: the byte-net engine of the device ABI (`engine.rs`, `plan.rs`, `queue.rs`, `dag.rs`, `instant.rs`, `advance.rs`, `tick.rs`, `observe.rs`, `DigitalLimits`). Depends only on `piperine-abi`; it names no analog type. |
| `piperine-api` | The library face (MD-20), rewritten small by `msl-cli` (its old content is gone): `Project` (a package found from `Piperine.toml`, its crate graph and its `[run.<name>]` tables; a lone file), `RunSpec`/`Overrides` (a run request, keys and flags), `Backend`, `Session` (one run: parse, type check, elaborate, compile on a backend once, run its analysis), `Probe`, `Table`, `Record`/`Streams` (`emit!` records decoded from the sink), `Plot`, `Verdict` (checks, `fatal`, target margins), `Failure` (`P0010`, `P0011`, `E0801` to `E0806`), and, from `host-ii`, `metadata.rs` (`Metadata`: params, ports, streams), `params.rs` (`Params`/`Derive`: a `set` that recomputes the derived params, MD-69) and the host drive, timed `set` and subscription of `Session` (`drive`, `set_at`, `subscribe`). Every rule of what a run prints or judges lives here, with a Rust test. |
| `piperine-cli` | The `piperine` binary, a shell over `piperine-api` (clap builder, no derive): the eight commands `check`, `build`, `run`, `test`, `info`, `new`, `clean`, `fmt`; text tables, record lines, CSV, JSON Lines and SVG in `render/`; exit codes 0/1/2. Its e2e targets run the real binary; `tests/book.rs` holds every CLI listing of the Book to the binary. |
| `piperine-project` | `Piperine.toml` (manifest, `[run]` tables accepted and located but not read: `piperine-api` reads them), path and git dependencies, the lockfile (MD-58, MD-59). |
| `piperine-lang-server` | LSP server. **Outside the default build and deferred** (user, 2026-11-16): it still targets the retired front end and is not edited. |
| `piperine-plan` | What the two backends of an `Ir` share (MD-63): `BodyPlan` (a body's ABI shape and the table of what each function computes: `FunctionPlan`, `Mode`, `Source`, `Target`, `Term`) and `Netlist::circuit`, the design's instance table bound to whatever implements `Modules`. Depends on `piperine-ir`, `piperine-abi`, `piperine-solver`. |
| `piperine-transpile` | The compiled backend (MD-63): `Transpiler::new` checks and plans an `Ir`; the IR is printed as one readable Rust source, built by `rustc` into a `cdylib` exporting `piperine_abi_1`, cached by hash and loaded. Generated code links only `piperine-abi` and the embedded runtime `rt`. |
| `piperine-parity` | The parity harness and the runner of MSL from source (`publish = false`): runs ngspice on `tests/ngspice/*.cir` at test time and compares the MSL twins under one `ToleranceContract`; `Project` takes a crate graph from disk to a compiled circuit, which the `buck_*` targets use to run `examples/buck` from its `.msl`. |

The root package `piperine` is the shell over `piperine-api` (`pub use piperine_api::*`, so a Rust
host writes `use piperine::…`); it builds, and its integrated tests are queue item 4 of `ROADMAP.md`.

## Solver architecture (current state)

- **One ABI:** the `Device` trait of `piperine-abi` (MD-62). The analog `Assembly` and the
  digital `Digital` run its shapes; the solver owns the companion, the limit iterate, every
  history and every checkpoint, and a device is `eval` over declared reads, writes and
  partials, and `react`. Devices are bound by `CircuitBuilder::device` (`core/bind.rs`).
- **Naming:** `Net` (`core/net.rs`) unifies analog nodes, branch currents, digital nets,
  and pseudo variables under one public identity with stable labels.
- **Convergence:** `ConvergencePlan` (analog crate, `convergence.rs`) composes `HomotopyStrategy`
  (gmin stepping, source stepping), `NewtonStrategy`, `StepperStrategy`, and `PlanLimits`
  (caps extracted from magic numbers).
- **Integration:** `TrBdf2` (analog crate, `math/integration.rs`) — the sole transient scheme (a
  Trapezoidal stage over `γh` then a BDF2 stage over `(1−γ)h`, γ = 2−√2); no
  method-selection surface. LTE-driven timestep via the plan's PI controller.
- **Errors:** `SolverDomain` enum — typed domains, no free strings.
- **Protocol:** the owner protocol (`protocol/`) steps both sides; a boundary change is solved on the left value, the instant runs, then it is delivered with a restart (MD-62). The digital engine returns `Result<_, DigitalError>`; the owner maps it to `Error` at its call sites.
- **Prelude:** `prelude.rs` exports the host-facing surface.

## Known gaps

- The hosts build and run an MSL project end to end: `piperine run` in `examples/rc` or
  `examples/buck`. The language server does not build and is deferred. What runs, what is refused
  and what is wrong today: `GAPS.md`. Planned order: `ROADMAP.md`.
- Solver ABI work in progress: `.specs/STATE.md` holds the decision log and
  `.specs/features/` the feature specs.

## Naming & conventions

- `gnd` is the reference node (the MNA ground).
- Elaboration folds every param to a constant per instance, a child's param that reads the
  parent's params included; the instance table holds the constants.
- A hoisted name keeps its IR path (`gate.x`, `above$0.init$0.up`) on its ABI pin; the
  generated Rust maps `.` to `_d` and `$` to `_s`.

## Declared language surface

The language's built-in items (the primitive value types, the math functions, the catalogue
of methods per receiver type, the environment reads) are tables of the front end; the
standard library is ordinary MSL source embedded in `piperine-lang` (`std/*.msl`). A name
that resolves to nothing, or a method with no catalogue row for its receiver's type, is a
diagnostic, never a silent no-op.

## Files not to edit casually

- `crates/piperine-lang/src/msl/` — the MSL lexer and parser; changes ripple through all
  parsing. Grammar and semantics come from `docs/spec/` only.
- `crates/piperine-lang/src/pom/` — the POM and its resolution; the elaborator reads it and
  never writes it.
- `crates/piperine-lang/std/` and `examples/buck/src/` — the acceptance targets of the front
  end: a form of theirs that does not check, elaborate or run is fixed in the compiler.
- `tests/fixtures*` — frozen test corpora.

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

- `docs/spec/` (Parts I–VIII + appendices A–G; `index.md` is the map) — the Piperine
  mixed-signal language (MSL) specification, the sole grammar and semantics source of the
  front end. Design record (adversarial critic/builder rounds, review):
  `.specs/features/msl-spec/`.
- What does not work yet, ranked: `GAPS.md`.
- Solver gaps + ABI plan: merged into `ROADMAP.md` (P1/P2)
- Spec-driven feature tracking: `.specs/STATE.md` + `.specs/features/`
- Open items: `ROADMAP.md`

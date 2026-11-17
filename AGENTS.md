# Piperine — Agent Instructions

Briefing for AI coding agents. `CLAUDE.md` is the authoritative companion (pipeline
diagram, crate table, known gaps) — read both before making changes.

## What Piperine is

A mixed-signal language and simulator. `piperine-lang` parses the Piperine mixed-signal
language (MSL, `docs/spec/` is its only grammar and semantics source) into a POM `Design`,
type-checks the M1 analog subset and the digital and mixed forms of `msl-digital` (`types/`)
and elaborates them into `piperine-ir`'s `Ir` (`elab/`, MD-64). `piperine-transpile` prints the
IR as Rust and builds it into a device library with `rustc` (MD-63); `examples/buck` runs whole
from its `.msl`. The native solver is three crates:
`piperine-solver` (analyses and the owner protocol over the device ABI) owning `piperine-analog-solver` and
`piperine-digital-solver`, which meet only through events at an instant, run by the owner protocol (MD-62). PHDL was retired in full
(MD-44/MD-45).

**The root `Cargo.toml`'s `default-members` are the crates that compile**, the host crates
`piperine-project`, `piperine-api`, `piperine-cli` and the root `piperine` shell package included
(feature `msl-cli`). Only `piperine-lang-server` stays outside the default build: it does not build,
is not edited, and is deferred by the user to a feature of its own.

```
source ──► piperine-lang ──► Design (POM) ──► Ir (piperine-ir) ──► piperine-transpile (Rust, rustc)
                                                  (piperine-plan)      │
                       piperine-solver (owner) ◄───────────────────────┘
                       ├── piperine-analog-solver
                       └── piperine-digital-solver
                                   │
                  hosts: piperine-api (Project, Session) ◄── piperine-cli (`piperine`)
```

Dependency direction: the owner `piperine-solver` depends on both sub-solvers and on
`piperine-abi`; the sub-solvers depend on neither the owner nor each other; events at an
instant are the boundary.
`piperine-abi` (the device ABI, MD-61) depends on no crate and is the only in-tree crate a
solver crate may depend on besides those arrows. Breaking either arrow is a regression.

## Build and verify

Always build and test before declaring work done:

```sh
cargo build     # the default members; zero warnings is the bar
cargo test      # their whole suite, ngspice parity included; never `--workspace` (the language server does not build)
```

## Workspace map

```
crates/
├── piperine-lang/            parser + type checker + elaborator: msl/ pom/ types/ elab/ (+ std/), msl::format (fmt)
├── piperine-ir/              the IR: arenas, builder, check(), printer and parser
├── piperine-abi/             device ABI: repr(C) contract, Device trait, thunks, caller::Handle, validate, verify
├── piperine-solver/          owner: protocol/, analyses/, bind (core/), Net naming, prelude
├── piperine-analog-solver/   analog sub-solver: math/, device/ (Assembly), convergence, TrBdf2
├── piperine-digital-solver/  digital sub-solver (the byte-net engine of the device ABI)
├── piperine-api/             the library face, small: Project, RunSpec, Session, Probe, Table, Record, Plot, Verdict
├── piperine-cli/             the `piperine` binary, a shell over piperine-api: check build run test new clean fmt
├── piperine-project/         Piperine.toml (+ [run] tables, unread) + dependency resolver + lockfile
├── piperine-lang-server/     LSP server: outside the default build, deferred (does not build)
├── piperine-plan/             the plan both backends share: BodyPlan (shape + function table), Netlist::circuit (MD-63)
├── piperine-transpile/       the compiled backend: IR printed as Rust, built by rustc, cached, loaded (MD-63)
└── piperine-parity/          parity harness (ngspice vs the MSL twins) and MSL from source (`Project`, buck_*) (test-only)
```

## Hard rules

- **Fail loud.** A construct outside the supported subset is a diagnostic naming it (`P0001`
  at type check or elaboration, `Unsupported` in the transpiler); unknown nets, params, or
  instances are loud errors at the host boundary. Never emit a silent `0.0` or a no-op.
- **No `unwrap()`/`expect()`** on user-input paths (LSP protocol I/O included — every
  request id must receive a response).
- **Frozen corpora:** `tests/fixtures*` — do not edit.
- **The parser** (`piperine-lang/src/msl/`) — change only with tests proving intent.

### Rust idiom rules (binding — recorded as MD-13)

These five rules govern every line of solver and codegen code. A PR that
violates any of them is not ready.

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

## Test placement

A target lives in the crate it exercises (MD-28); the root `tests/` holds no Rust target.
Do not maintain a file list anywhere — enumerate with `ls crates/*/tests/*.rs` and read
each target's `//!` scope header. `crates/piperine-lang/tests/suite_hygiene.rs` enforces
the mechanical half of the rule.

## Documentation

- `docs/spec/` — the Piperine mixed-signal language (MSL) specification, the sole grammar
  and semantics source of the front end; design record in `.specs/features/msl-spec/`
- What does not work yet, ranked: `GAPS.md`
- Solver gaps + ABI plan: merged into `ROADMAP.md` (P1/P2)
- Spec-driven feature tracking: `.specs/STATE.md` + `.specs/features/`
- Open items: `ROADMAP.md`

# Piperine — Agent Instructions

Briefing for AI coding agents. `CLAUDE.md` is the authoritative companion (pipeline
diagram, crate table, known gaps) — read both before making changes.

## What Piperine is

A mixed-signal language and simulator. `piperine-lang` parses and elaborates the source
(Verilog-AMS LRM 2.4, `.vams`/`.va`, Annex A grammar only, C7 — never a third-party
Verilog-A compiler) into a POM `Design`. The native solver is three crates:
`piperine-solver` (analyses and the `Element` ABI) owning `piperine-analog-solver` and
`piperine-digital-solver`, which meet through events as the target boundary (the digital snapshot stays shared until feature B). PHDL was retired in full
(MD-44/MD-45).

**Only `piperine-lang` and the three solver crates compile today.** The root `piperine`
shell package, `piperine-project`, `piperine-api`, `piperine-cli` and `piperine-lang-server` stay in the tree but do not
build until the new device compiler lands.

```
source ──► piperine-lang ──► Design (POM) ──► device compiler (to come)
                                                   │
                       piperine-solver (owner) ◄───┘
                       ├── piperine-analog-solver
                       └── piperine-digital-solver
                                   │
                  hosts: piperine-api (Rust) / piperine-cli
```

Dependency direction: the owner `piperine-solver` depends on both sub-solvers; the
sub-solvers depend on neither the owner nor each other; events are the target boundary.
`piperine-solver` depends on no other in-tree crate. Breaking either arrow is a
regression.

## Build and verify

Always build and test before declaring work done:

```sh
cargo build     # the default members: lang + the three solver crates; zero warnings is the bar
cargo test      # their whole suite; never `--workspace` (four crates do not build yet)
```

## Workspace map

```
crates/
├── piperine-lang/            parser + elaborator: grammar/ elab/ pom/ eval/ (+ headers/)
├── piperine-solver/          owner: Element ABI (core/), analyses/, Net naming, prelude
├── piperine-analog-solver/   analog sub-solver: math/, convergence, TrBdf2
├── piperine-digital-solver/  digital sub-solver (event-driven scheduler)
├── piperine-api/             the library face: session, results, waveform (does not build yet)
├── piperine-cli/             `piperine` CLI (does not build yet)
├── piperine-project/         Piperine.toml + dependency resolver (does not build yet)
└── piperine-lang-server/     LSP server (does not build yet)
```

## Hard rules

- **Fail loud.** Unlowered constructs return `CodegenError::Unsupported`; unknown nets,
  params, or instances are loud errors at the host boundary. Never emit a silent `0.0`
  or a no-op.
- **No `unwrap()`/`expect()`** on user-input paths (LSP protocol I/O included — every
  request id must receive a response).
- **Frozen corpora:** `headers/`, `tests/fixtures*` — do not edit.
- **Hand-written parsers** (`piperine-lang/src/{grammar,lexer.rs,parser.rs}`) — change only with tests proving
  intent.

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

- Verilog-AMS language reference: `VAMS-LRM-2-4.md` (the sole grammar/semantics source)
- `docs/spec/` — the Piperine mixed-signal language (MSL) specification, the target
  design for the next frontend (not a description of the current VAMS frontend);
  design record in `.specs/features/msl-spec/`
- Solver gaps + ABI plan: merged into `ROADMAP.md` (P1/P2)
- Spec-driven feature tracking: `.specs/STATE.md` + `.specs/features/`
- Open items: `ROADMAP.md`

# Piperine — Agent Instructions

Briefing for AI coding agents. `CLAUDE.md` is the authoritative companion (pipeline
diagram, crate table, known gaps) — read both before making changes.

## What Piperine is

A hardware-description language (PHDL, `.phdl`) and simulator for analog and mixed-signal
circuits. The frontend elaborates to a POM `Design`, which codegen resolves into its own
private form and compiles via Cranelift JIT (analog) + an event-driven interpreter
(digital) into the solver's **`Element`** ABI. Verilog-A device models load as compiled
OSDI (v0.4) shared libraries through an external plugin. Simulations are driven by
**hosts**: Python (`import piperine`, the scripting host) or Rust (`piperine-api`, the
library face — MD-20; the root `piperine` crate is a thin re-export shell over it).

```
.phdl ──► piperine-lang (parse/ elab/) ──► Design (POM)
                                           │
                                           ▼
                    piperine-codegen (resolve/ → flatten/ → emit/ → kernel/ → device/)
                                           │
                                           ▼
                              piperine-solver (Element ABI, DC/AC/tran/noise/TF)
                                           │
                                           ▼
                              hosts: `piperine-api` (Rust) / `import piperine` (Python)
```

Dependency direction (MD-20): `api → {lang, codegen, solver}`; `python → api`;
`root(shell) → api`; `cli → {python, api, project}`. And
**`piperine-solver` never depends on `piperine-codegen`** — the codegen lowers *into* the
solver's types. Breaking either arrow is a regression.

## Build and verify

Always build and test before declaring work done:

```sh
cargo build --workspace     # zero warnings is the bar
cargo test  --workspace     # the whole suite; bare `cargo test` only runs the root package
```

## Workspace map

```
src/lib.rs                root `piperine` shell (MD-20): `pub use piperine_api::*`
│                         and nothing else; root tests/ = host + cross-crate suites
crates/
├── piperine-lang/          PHDL frontend: parse/ elab/ pom/ eval/ (+ headers/)
├── piperine-codegen/       POM → devices, one module per pipeline stage (MD-23):
│                           resolve/ (private resolved form + diff) → flatten/ → emit/
│                           (Cranelift) → kernel/ (AnalogKernel, DigitalKernel) →
│                           device/ (AnalogInstance, CircuitCompiler → PiperineDevice)
├── piperine-solver/        Element ABI (core/), analyses/ (DC/AC/tran/noise/TF, PSS, PZ,
│                           SP, disto, sens, ConvergencePlan), math/ (MNA, integration),
│                           digital/ (scheduler), Net naming, prelude
├── piperine-api/           the library face (MD-20): session, results, waveform, fourier,
│                           units, hooks, error, prelude
├── piperine-python/        the `import piperine` binding layer over piperine-api
├── piperine-plugin/        plugin SDK + host: native dlopen + embedded Python, TOFU +
│                         permissions consent, @device, release-asset device binaries
├── piperine-plugin-macros/ #[pip::device] / #[pip::script] / #[pip::hook(phase)]
├── piperine-cli/           `piperine` CLI (check, fmt, run, test, new, add, remove, tree, plugin)
├── piperine-project/       Piperine.toml + git dependency resolver + plugin lockfile
└── piperine-lang-server/   LSP server (editors/vscode/ is the extension)
```

## Hard rules

- **Fail loud.** Unlowered constructs return `CodegenError::Unsupported`; unknown nets,
  params, or instances are loud errors at the host boundary. Never emit a silent `0.0`
  or a no-op.
- **No `unwrap()`/`expect()`** on user-input paths (LSP protocol I/O included — every
  request id must receive a response).
- **Frozen corpora:** `headers/`, `tests/fixtures*` — do not edit.
- **Hand-written parsers** (`piperine-lang/src/parse/`) — change only with tests proving
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

A target lives in the crate it exercises (MD-28); root `tests/` is for the host surface
and cross-crate proofs. Do not maintain a file list anywhere — enumerate with
`ls crates/*/tests/*.rs tests/*.rs` and read each target's `//!` scope header.
`tests/suite_hygiene.rs` enforces the mechanical half of the rule.

| What | Where |
|------|-------|
| host API (session/results/waveform) | root `tests/session*.rs`, `tests/host_*.rs` |
| example gallery (dual contract) | root `tests/run_examples.rs` (every `.phdl` elaborates + every `.py` runs) |
| ngspice cross-validation | root `tests/ngspice_validation.rs` (+ `tests/ngspice/`) |
| syntax/elaboration gates | `piperine-lang/tests/{parse_elab,elab,bench_removed}.rs` |
| POM → resolved + circuit | `piperine-codegen/tests/{resolve_lowering,codegen_api,circuit_from_design,silent_bugs}.rs` |
| JIT kernels | `piperine-codegen/tests/{analog_kernel,digital_jit}.rs` |
| solver analyses / mixed-signal | `piperine-solver/tests/{digital_topology,mixed_signal}.rs` |
| plugin e2e | `piperine-plugin/tests/{e2e,native_smoke,inject,staging,hooks,scripts,release_fetch,macro_collisions,extern_stub,trust}.rs`, `piperine-plugin-macros/tests/`, root `tests/plugin_parity.rs` |

## Documentation

- Formal spec: `docs/spec/` (Parts I–VII + appendices A/B)
- Solver gaps + ABI plan: merged into `ROADMAP.md` (P1/P2)
- Spec-driven feature tracking: `.specs/STATE.md` + `.specs/features/`
- Open items: `ROADMAP.md`

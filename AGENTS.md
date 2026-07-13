# Piperine — Agent Instructions

Briefing for AI coding agents. `CLAUDE.md` is the authoritative companion (pipeline
diagram, crate table, known gaps) — read both before making changes.

## What Piperine is

A hardware-description language (PHDL, `.phdl`) and simulator for analog and mixed-signal
circuits. The frontend elaborates to a POM `Design`, which codegen lowers (private resolved
form — no separate IR crate) and compiles via Cranelift JIT (analog) + an event-driven
interpreter (digital) into the solver's **`Element`** ABI. Verilog-A device models load as
compiled OSDI (v0.4) shared libraries through an external plugin. The `bench` layer
interprets verification code over the elaborated design and drives the solver.

```
.phdl ──► piperine-lang (parse/elab) ──► Design (POM)
                │                              │
                ▼                              ▼
        piperine-bench (BenchRunner)   piperine-codegen (lower/ + jit/ + device/)
                │                              │
                └──────────► piperine-solver ◄─┘
                             (Element ABI, DC/AC/tran/noise/TF)
```

Dependency direction: **`piperine-solver` never depends on `piperine-codegen`** — the
codegen lowers *into* the solver's types. Breaking the arrow is a regression.

## Build and verify

Always build and test before declaring work done:

```sh
cargo build --workspace     # zero warnings is the bar
cargo test  --workspace     # 51 green targets; bare `cargo test` only runs the root package
```

## Workspace map

```
crates/
├── piperine-lang/          PHDL frontend: parse/ elab/ pom/ eval/ (+ headers/)
├── piperine-codegen/       POM → devices: lower/ (private resolved form), jit/ (analog +
│                           digital kernels), device/ (AnalogInstance, DigitalInstance,
│                           CircuitCompiler → PiperineDevice)
├── piperine-solver/        Element ABI, Net naming, ConvergencePlan, IntegrationMethod,
│                           DC/AC/tran/noise/TF drivers, digital scheduler, prelude
├── piperine-bench/         bench runtime: SimHost, BenchTasks, result objects, BenchRunner
├── piperine-plugin/        plugin SDK + host: native/WASM/process backends, TOFU, @device
├── piperine-plugin-wasm/   WASM guest SDK
├── piperine-cli/           `piperine` CLI (check, fmt, run, test, new, add, remove, tree, plugin)
├── piperine-project/       Piperine.toml + git dependency resolver + plugin lockfile
└── piperine-lang-server/   LSP server (editors/vscode/ is the extension)
```

## Hard rules

- **Fail loud.** Unlowered constructs return `CodegenError::Unsupported`; unimplemented
  bench tasks are elaboration errors via the `bench_task_implemented` allowlist
  (`piperine-lang/src/eval/tasks.rs`). Never emit a silent `0.0` or a no-op.
- **Allowlist discipline.** A new bench task needs the allowlist entry *and* a `BenchTask`
  impl (`piperine-bench/src/tasks.rs`) in the same change, plus the bench spec §11 row
  (`crates/piperine-bench/docs/SPEC.md`).
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

| What | Where |
|------|-------|
| bench e2e behavior | `piperine-bench/tests/bench.rs` (`elab` helper + `CIRCUIT` fixture) |
| example gallery | `piperine-bench/tests/run_examples.rs` (every `examples/*.phdl` stays green) |
| syntax/elaboration gates | `piperine-lang/tests/{parse_elab,bench,elab}.rs` |
| POM → resolved + circuit | `piperine-codegen/tests/{codegen_ir,codegen_api,from_ir,silent_bugs}.rs` |
| JIT kernels | `piperine-codegen/tests/{analog_jit,digital_jit}.rs` |
| solver analyses / mixed-signal | `piperine-solver/tests/{digital_topology,mixed_signal}.rs` |
| plugin e2e | `piperine-plugin/tests/{e2e,native_smoke,phase3,process_smoke,wasm_smoke,trust,manifest}.rs` |

## Documentation

- Formal spec: `docs/spec/` (Parts I–VII + appendices A/B)
- Solver gaps + ABI refactor plan: `SOLVER_GAPS.md`
- Spec-driven feature tracking: `.specs/STATE.md` + `.specs/features/`
- Open items: `ROADMAP.md`

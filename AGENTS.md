# Piperine — Agent Instructions

Briefing for AI coding agents. `CLAUDE.md` is the authoritative companion (pipeline
diagram, crate table, known gaps) — read both before making changes.

## What Piperine is

A hardware-description language (PHDL, `.phdl`) and simulator for analog and mixed-signal
circuits. The frontend elaborates to a POM `Design`, lowers to a shared IR
(`piperine-ir`), which compiles via Cranelift JIT (analog) + an event-driven interpreter
(digital) into the solver's `Device` trait. Verilog-A device models load as compiled OSDI
(v0.4) shared libraries. The `bench` layer interprets verification code over the
elaborated design and drives the solver.

```
.phdl ──► piperine-lang (parse/elab/lowering) ──► IrProgram
                │                                     │
                ▼                                     ▼
        piperine-bench (BenchRunner)         piperine-codegen (jit/ + device/)
                │                                     │
                └────────────► piperine-solver ◄──────┘
                               (DC/AC/tran/noise/TF, OSDI)
```

Dependency direction: **`piperine-solver` never depends on `piperine-codegen`** — the
codegen lowers *into* the solver's types. Breaking the arrow is a regression.

## Build and verify

Always build and test before declaring work done:

```sh
cargo build --workspace     # zero warnings is the bar
cargo test  --workspace     # 45 green targets; bare `cargo test` only runs the root package
```

## Workspace map

```
crates/
├── piperine-lang/          PHDL frontend: parse/ elab/ pom/ lowering/ eval/ (+ headers/)
├── piperine-ir/            shared IR: expr, stmt, symbols, validate, symbolic diff
├── piperine-codegen/       IR → devices: jit/ (flatten, analog kernel, emit, digital)
│                           and device/ (AnalogInstance stamping, CircuitCompiler)
├── piperine-solver/        Newton-Raphson MNA, analysis/ (dc, ac, transient, noise, tf),
│                           math/ (faer), osdi/ loader
├── piperine-bench/         bench runtime: SimHost, BenchTasks, result objects, BenchRunner
├── piperine-cli/           `piperine` CLI (check, fmt, run, test, new, add, remove, tree)
├── piperine-project/       Piperine.toml + git dependency resolver
└── piperine-lang-server/   LSP server (editors/vscode/ is the extension)
```

## Hard rules

- **Fail loud.** Unlowered IR constructs return `CodegenError::Unsupported`; unimplemented
  bench tasks are elaboration errors via the `bench_task_implemented` allowlist
  (`piperine-lang/src/eval/tasks.rs`). Never emit a silent `0.0` or a no-op.
- **Allowlist discipline.** A new bench task needs the allowlist entry *and* a `BenchTask`
  impl (`piperine-bench/src/tasks.rs`) in the same change, plus the bench spec §11 row
  (`crates/piperine-bench/docs/SPEC.md`).
- **No macro magic.** Data tables + plain helpers. Every helper has an owner (struct
  method or extension trait) — no loose module-level fns.
- **No `unwrap()`/`expect()`** on user-input paths (LSP protocol I/O included — every
  request id must receive a response).
- **Frozen corpora:** `headers/`, `tests/fixtures*` — do not edit.
- **Hand-written parsers** (`piperine-lang/src/parse/`) and the IR contract
  (`piperine-ir/src/`) — change only with tests proving intent.

## Test placement

| What | Where |
|------|-------|
| bench e2e behavior | `piperine-bench/tests/bench.rs` (`elab` helper + `CIRCUIT` fixture) |
| example gallery | `piperine-bench/tests/run_examples.rs` (every `examples/*.phdl` stays green) |
| syntax/elaboration gates | `piperine-lang/tests/{parse_elab,bench}.rs` |
| POM → IR | `piperine-lang/tests/{ppr_ir,codegen_ir}.rs` |
| JIT kernels | `piperine-codegen/tests/{analog_jit,digital_jit}.rs` |
| solver analyses / OSDI | `piperine-solver/tests/` (OSDI needs `OPENVAF_BIN` in PATH) |

## Documentation

- Language spec: `crates/piperine-lang/docs/SPEC.md`
- Bench spec: `crates/piperine-bench/docs/SPEC.md`
- IR spec: `crates/piperine-codegen/docs/SPEC.md`
- Open work: `ROADMAP.md`

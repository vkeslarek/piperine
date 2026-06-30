# Piperine

> ⚠️ **Work in progress — not production ready.** APIs, syntax, and behavior
> change without notice. The implementation covers a meaningful subset of the
> language design (see [`docs/GAPS.md`](docs/GAPS.md) for what is *not* yet
> implemented). Use it to explore and contribute, not for anything you
> depend on.

A hardware-description language and simulator for analog and mixed-signal
circuits. Both Verilog-A/AMS (`.va`, `.vams`) and a new native HDL called
**PHDL** (`.phdl`, `.ppr`) lower to a shared IR, which compiles to a
pure-Rust Newton–Raphson solver. The solver also accepts optional
`.osdi` device models compiled by OpenVAF-Reloaded for industrial device
libraries (BSIM, EKV, PSP, …).

## Architecture

```
   .va / .vams    ──►  piperine-ams    ──►      ┌──────────────────┐
   (Verilog-A/AMS)  ◄─►   frontend     ◄─►      │  piperine-codegen │
                                                  │   (IR + lowering)  │
   .phdl / .ppr    ──►  piperine-lang  ──►      └────────┬─────────┘
   (PHDL / .ppr)     ◄─►   frontend     ◄─►               │
                                                            ▼
                                                  Vec<Box<dyn Device>>
                                                            │
                                                  ┌─────────┴─────────┐
                                                  ▼                   ▼
                                       ┌──────────────────┐  ┌──────────────────────┐
                                       │  piperine-solver   │  │  piperine-solver OSDI │
                                       │  (Newton-Raphson,  │  │  (.osdi shared libs)  │
                                       │   AC/DC/Tran/      │  │                        │
                                       │   Noise/TF)         │  └──────────────────────┘
                                       └──────────────────┘
```

The **IR (`crates/piperine-codegen/src/ir.rs`) is the only contract** between
the frontends and the solver. `piperine-solver` does *not* depend on
`piperine-codegen`; the codegen depends on the solver's `Device` and
`CircuitInstance` traits because it lowers IR into them. Verify this
dependency direction with `cargo metadata` if you change it.

## Crates

| Crate | Role | Key files |
|-------|------|-----------|
| `piperine-ams` | Verilog-A/AMS frontend | `src/{lexer,parser,preprocessor,grammar,ast,model,fmt}.rs`, `headers/*.vams` |
| `piperine-lang` | PHDL frontend (parse + elab) | `src/parse/`, `src/elab/`, `src/resolve/`, `src/stdlib/` |
| `piperine-codegen` | IR + Cranelift JIT + lowering to `Device` | `src/ir.rs`, `src/from_ams.rs`, `src/from_ppr.rs`, `src/from_ir.rs`, `src/ir_analog_to_device.rs`, `src/ir_digital_to_interp.rs`, `src/phdl_device.rs`, `src/codegen/` |
| `piperine-solver` | Newton-Raphson, AC/DC/Tran/Noise/TF, OSDI loader | `src/{analog,digital,osdi,solver,math,topology}.rs` |
| `piperine-cli` | clap subcommands | `src/commands/{check,fmt,build,run,test,new,clean}.rs` |
| `piperine-project` | `Piperine.toml` reader | `src/lib.rs` |

## Quick start

```sh
cargo build                          # build the workspace
cargo test                           # ~270 tests; must pass (see tests-baseline.md)
cargo test -p piperine-codegen        # always re-run after touching codegen
```

### Verifying a design

```sh
# Parse + elaborate + sanity-check a PHDL or Verilog-A file
cargo run -p piperine-cli -- check path/to/circuit.phdl

# Format (token-level pretty-printer for Verilog-A)
cargo run -p piperine-cli -- fmt path/to/circuit.vams
```

The other CLI subcommands (`build`, `run`, `test`, `clean`) are stubs that
print "TODO: call simulator/compiler". See [`docs/GAPS.md`](docs/GAPS.md)
for the planned scope.

### OSDI tests

The OSDI subset of `piperine-solver` tests loads real `.osdi` device models
produced by OpenVAF-Reloaded. `OPENVAF_BIN` is auto-downloaded by
`piperine-solver/build.rs` on linux x86_64. On other platforms the build
script falls back to a system `openvaf`.

## Where to read next

| Doc | What it covers |
|-----|----------------|
| [`AGENTS.md`](AGENTS.md) | Build/test commands, frozen-file rules, conventions, test baseline |
| [`docs/piperine-hdl-spec.md`](docs/piperine-hdl-spec.md) | The PHDL language design |
| [`crates/piperine-codegen/IR-SYSTEM.md`](crates/piperine-codegen/IR-SYSTEM.md) | The IR contract between frontends and solver |
| [`docs/GAPS.md`](docs/GAPS.md) | Spec-vs-code gap analysis; the authoritative development guide |

## IR in 30 seconds

`IrProgram { source, modules, functions }`. Each `IrModule` has ports,
params, wires, branches, vars, instances, connections, an optional
`analog` body, and an optional `digital` body. Analog statements are
`Contrib` / `Force` / `IndirectContrib`; digital statements mirror
Verilog-A's `initial`/`always` (`Assign` / `NonBlocking` / `EventControl`).
`IrExpr` covers literals, params, vars, branch access (`V(a,b)` /
`I(a,b)`), all arithmetic, `Call` (math), `Sim` queries
(`$temperature`, `$vt`, `$abstime`, …), and reactive `StateRef`s. See
`IR-SYSTEM.md` for the full grammar and semantics.

The IR is emitted to Cranelift JIT code that takes `(node_voltages,
params, sim_ctx, rhs)` (or `jac`/`charge`/`charge_jacobian` for the
derivative/companion-model variants). The 4th argument, `SimCtx`, is a
32-byte struct carrying the live simulator state — see A.2/A.3 in GAPS
for the threading story.

## Conventions

- **Panics:** never `unwrap()`/`expect()` on user-input paths; return
  `Result<_, E>`. `unwrap()` is acceptable only behind a provable
  invariant (`peek`-guarded lexer reads, FFI length-checked slices), and
  should carry a `// SAFETY:` comment.
- **Fail-loud over silent zero:** the codegen uses
  `CodegenError::Unsupported(...)` rather than `todo!()`/`unimplemented!()`
  and rejects IrExpr constructs that would silently compile to a wrong
  value. See `validate_ir_contrib` in `ir_emit.rs`.
- **Numeric conventions:** analog = `f64`; digital = `LogicValue` (`Zero`,
  `One`, `X`, `Z`); mixed-signal nets = anonymous `usize` indices.
- **No new dependencies** without checking `Cargo.toml` and the
  workspace `[workspace.dependencies]` table first.

## Status

This is a **work in progress**. The current implementation is roughly:

- **AMS frontend** — full Verilog-A/AMS grammar parses; digital
  (`initial`/`always`) and generate blocks are not yet lowered to IR
  (`digital: None` hardcoded).
- **PHDL frontend** — parses and elaborates most of the spec; generics,
  capabilities, bundles, enums, and higher-order functions are
  parse-only or partial.
- **Codegen** — handles resistive `I(p,n) <+ …` contributions and `ddt`
  via the companion model. Forces (`V(p,n) <-`), indirect contributions,
  and most analog operators are rejected with clear errors.
- **Solver** — DC, AC, Tran (backward Euler only), Noise (adjoint),
  Transfer Function. Trapezoidal integration and LTE-based timestep control
  are defined but not wired into the transient loop.
- **Mixed-signal** — A2D and D2A bridges are not yet implemented; the
  solver passes empty analog voltages into `eval_discrete` and the digital
  state is invisible to analog stamping.
- **OSDI** — fully functional via OpenVAF-Reloaded.

See [`docs/GAPS.md`](docs/GAPS.md) for the full gap analysis with
file:line citations, proposed solutions, and acceptance criteria for each
gap.
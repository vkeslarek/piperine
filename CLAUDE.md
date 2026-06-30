# Piperine — Claude Code Instructions

## Project summary

Piperine is a Verilog-AMS / Verilog-A superset plus a PHDL (`.ppr`) hardware-description
language, compiled through a shared **IR** to a **native in-house circuit solver**
(Cranelift-JIT analog devices + an event-driven digital interpreter). There is no
external SPICE dependency for the IR path; OSDI shared libraries are also supported.
The project is a Rust workspace (edition 2024).

## Pipeline (the spine)

```
Verilog-AMS (.va/.vams) ──ams_to_ir──┐
                                     ├──► IrProgram ──from_ir──► CircuitInstance ──► solver
PHDL (.ppr/.phdl) ──ppr_to_ir────────┘                (analog: Cranelift JIT,
                                                        digital: interpreter)
```

The **IR** (`crates/piperine-codegen/src/ir.rs`) is the superset both frontends lower into.
"100% coverage" means: every AMS/PHDL construct maps to IR, and every IR construct lowers
to executable device code. When something cannot be faithfully lowered, **fail loud**
(`CodegenError::Unsupported`) — never silently emit `0.0`.

## Build and test

```sh
cargo build                       # build all crates
cargo test                        # run the whole suite (~247 tests)
cargo test -p piperine-codegen    # one crate
cargo test <name>                 # one test
cargo test -- --nocapture         # see solver output
```

No worker/IPC build step — the solver is in-process.

## Crate responsibilities

| Crate | Role |
|-------|------|
| `piperine-ams` | Verilog-AMS frontend: preprocessor, lexer, recursive-descent parser → AST (`grammar/`, `ast/`), `Document::parse[_file]`, formatter (`fmt`). |
| `piperine-lang` | PHDL frontend: lexer/parser (`parse/`), name resolution (`resolve/`), elaboration → `elab::ir::ElabProgram` (`elab/`). `parse_and_elaborate`. |
| `piperine-codegen` | The IR (`ir.rs`) + both lowerings (`from_ams`, `from_ppr`) + IR→device (`from_ir`, `ir_analog_to_device`, `ir_digital_to_interp`) + Cranelift/interp codegen (`codegen/`). |
| `piperine-solver` | Native solver: DC/AC/transient/noise/TF analyses (`analysis/`, `solver/`), MNA/linear algebra (`math/`, faer), `Device` trait, OSDI loader (`osdi/`), digital topology. |
| `piperine-cli` | `piperine` CLI: `check`, `build`, `run`, `fmt`, `new`, `test`, `clean`. |
| `piperine-project` | Project root / manifest discovery. |

## Key types and the analog device path

- `ir::IrProgram` → `ir::IrModule` (ports, params, wires, branches, `analog: IrAnalogBody`,
  `digital: IrDigitalBody`, instances, functions).
- `from_ir::from_ir(&IrProgram, top)` walks the top module's instances, resolves nets to
  `NodeIdentifier` (ground names: `gnd/GND/vss/VSS`), resolves params via `eval_ir_const`
  (compile-time folding of literal/param-ref/arithmetic/`Select`/math-call defaults), and
  builds a `CircuitInstance` of `PhdlDevice`s.
- `ir_analog_to_device` collects flow (`I`) contributions (through `if`/`case`) and JITs a
  residual + Jacobian via the shared Cranelift skeleton in `codegen/analog.rs`.

### `AnalogExpr` trait (`codegen/ir_emit.rs`)

The Cranelift residual/Jacobian skeleton is generic over `AnalogExpr` — three ops:
`emit` (→ Cranelift `Value`), `diff` (symbolic, w.r.t. a `V(p,n)` branch key),
`collect_branches`. Implemented for **`IrExpr`** (the IR front door — `codegen/ir_emit.rs`)
and for PHDL **`Expr`** (legacy `from_elab` path — delegates to `codegen/{expr,autodiff}`).
The IR emitter covers the full algebraic IR (literals, params, branch voltages, all
binary/unary ops, ternary `Select`, built-in math). `validate_ir_contrib` rejects anything
it cannot lower faithfully.

The Jacobian is **symbolic differentiation**, not autodiff (despite the filename
`autodiff.rs`). `diff_ir`/`diff` return a new expression that is emitted like any other.

## Known gaps / what's intentionally unsupported (fail loud)

The **full** Verilog-AMS analog-operator family already maps into the IR
(`from_ams`: `ddt`, `idt`, `idtmod`, `ddx`, `delay`/`absdelay`, `transition`, `slew`,
`laplace_*`, `zi_*`, `ac_stim`, all `$`-sysfuncs, noise via `scan_noise`). What remains is
per-operator **IR→code** lowering. Constructs not yet lowered return
`CodegenError::Unsupported` (named) rather than miscompiling:

- **`ddt` — DONE**: lowered via the companion model. `ir_analog_to_device` splits each
  contribution into a resistive part (`StateRef → 0`) and a charge `Q(V) = expr[StateRef→arg]
  − expr[StateRef→0]`. The JIT emits `charge`/`charge_jacobian` alongside residual/jacobian;
  `PhdlDevice::load_transient` stamps `alpha·dQ/dV` (`alpha = 1/dt`, companion source from the
  Norton transform), `load_ac` stamps `jω·dQ/dV`. Caps/inductors now contribute.
- **Other analog operators** (`idt`, `idtmod`, `ddx`, `transition`, `slew`, `laplace`,
  `zi`, `delay`) — recognised in the IR, fail loud at codegen. Each is its own follow-up.
- **Potential contributions** `V(p,n) <+ ...` (ideal voltage sources: vsource/vstep/vramp) —
  need MNA branch-current unknowns the nodal `JitAnalogDevice` doesn't have yet.
- **Noise** (`PhdlDevice::noise_current_psd`) returns empty; `IrNoiseSource` is populated but
  not yet stamped into AC noise analysis.
- **Indirect contributions**, bitwise/shift ops in contributions, non-`V`/`I` branch access.

The OSDI device (`solver/src/osdi/device.rs`) is the reference for reactive/noise stamping.

## Naming & conventions

- AMS module names: bare lowercase from the source (`resistor_va`, `nmos`, …).
- Ground net → MNA reference; gnd-family names listed above.
- Frontends: AMS keeps param defaults as IR expressions (folded later by `eval_ir_const`);
  PHDL pre-folds param defaults during elaboration.

## Files not to edit casually

- `crates/piperine-lang/src/parse/`, `crates/piperine-ams/src/grammar/` — hand-written
  recursive-descent parsers; changes ripple through all parsing.
- `crates/piperine-codegen/src/ir.rs` — the shared IR contract for both frontends.
- `crates/piperine-codegen/src/codegen/analog.rs` — the shared JIT residual/Jacobian skeleton.

## Tests of record

- `piperine-codegen/tests/wave1_nonlinear_tests.rs` — nonlinear/piecewise analog numerics
  (diode exp I-V, ternary `Select`, `**`) evaluated through the JIT device.
- `piperine-codegen/tests/codegen_e2e_tests.rs` — IR → solver DC/transient end-to-end.
- `piperine-codegen/tests/ams_ir_e2e_tests.rs`, `from_ir_tests.rs` — frontend → IR → device.
- `piperine-solver/tests/` — solver-level analyses, mixed-signal, OSDI, cosim.

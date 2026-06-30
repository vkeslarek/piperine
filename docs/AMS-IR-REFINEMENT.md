# Piperine — IR Architecture

## 1. Compilation Pipeline

Piperine does **not** evaluate Verilog-AMS at runtime. Every user-defined module
is compiled to a native Rust struct that implements `AnalogDevice` or
`DigitalDevice`. The runtime sees only those trait objects — no interpreter, no
VM, no reflection.

Codegen lives in `piperine-lang` alongside the IR — no separate crate.

```
Verilog-AMS (.va / .ppr)
      │
      ▼  piperine-parser
     AST (Document, Module, ...)
      │
      ▼  FrontendLower  (piperine-lang/src/lowering/)
   IrDesign
      │
      ▼  CodegenBackend  (piperine-lang/src/codegen/)
  Cranelift JIT  (pure-Rust — no external toolchain required)
      │
      ▼  JITModule::finalize_definitions()
native function pointers (residual, jacobian)
      │
      ▼  JitAnalogDevice wraps pointers + keeps JITModule alive
Box<dyn AnalogDevice>
Box<dyn DigitalDevice>
      │
      ▼  piperine-solver
  Simulation result
```

### External VA models (OSDI path)

Pre-compiled `.osdi` shared libraries (BSIM4, PSP, HiSIM, …) bypass the codegen
step — loaded directly via `dlopen`. Piperine **never** invokes OpenVAF; the
`.osdi` must be provided alongside the model.

```
external_model.osdi
      │
      ▼  dlopen  (piperine-solver/src/analog/osdi/)
Box<dyn AnalogDevice>   (OsdiDevice wrapped in DeviceRuntime)
```

---

## 2. Crate Responsibilities

| Crate | Role |
|---|---|
| `piperine-parser` | Verilog-AMS 2.4 lexer + recursive-descent parser → `Document` AST |
| `piperine-lang` | `IrDesign` data model · `FrontendLower` trait · `CodegenBackend` trait · Rust codegen impl |
| `piperine-solver` | KCL/KVL analog solver + digital DAG event scheduler; consumes `Box<dyn AnalogDevice>` / `Box<dyn DigitalDevice>` + OSDI loader |
| `piperine-project` | Project discovery, `.ppr` manifest, include-path management |
| `piperine-cli` | CLI binary — orchestrates parser → codegen → solver |

---

## 3. Codegen Backend — Decision and Rationale

### Options considered

**A. Generate Rust source** (`quote` + `proc_macro2`)

Walk `IrDesign`, emit `TokenStream` per module, write `.rs` file, compile with
`rustc --crate-type=cdylib`, `dlopen` the result.

**B. LLVM IR directly** (like OpenVAF, via `inkwell`)

Walk IR, emit LLVM IR instructions, compile to `.so` via LLVM AOT/JIT.

**C. Cranelift** (pure-Rust JIT) — **chosen**

Walk IR, emit Cranelift IR, JIT compile to native code in-process.
Ships entirely as a Rust library crate — no external toolchain, no C++ deps.

---

### Why Cranelift (Option C)

| Concern | Rust codegen | LLVM | Cranelift |
|---|---|---|---|
| External toolchain | rustc binary required at runtime | LLVM shared lib (C++ dep, version pinned) | **None — pure Rust crate** |
| C++ dependency | None | Yes (via inkwell) | **None** |
| Complexity to implement | Low (`quote!` ergonomic) | Very high | Medium — Cranelift IR is explicit but tractable |
| Correctness safety net | rustc checks generated code | Crash at runtime | Crash at runtime |
| Autodiff for Jacobian | Dual numbers in Rust | AD pass over LLVM IR | **Symbolic diff over `IrExpr`** (implemented) |
| Debuggability | Read the `.rs` file | LLVM IR | Cranelift IR (readable with `--print-ir`) |
| Runtime performance | Identical to hand-written Rust | Marginally faster | ~10–30% behind LLVM |
| Binary portability | Requires user to have rustc | Requires LLVM | **Self-contained** |

**Why not Rust codegen:** Requires `rustc` installed at simulation runtime, which
breaks "zero external deps" goal. Also means a subprocess, temp files, and a
`dlopen` round-trip per module.

**Why not LLVM:** C++ shared library, version-pinned, adds 50+ MB to install.
OpenVAF uses it to produce portable `.osdi` files; we don't need that.

**Chosen: Cranelift.** Ships inside the binary as a pure Rust dep. The
`piperine-lang` crate compiles `IrAnalogBlock` to native function pointers at
simulation startup — no external process, no temp files, no `dlopen`.
Autodiff is implemented as symbolic differentiation over `IrExpr`
(`piperine-lang/src/codegen/autodiff.rs`) — exact, inspectable, and free of
floating-point overhead.

**Status:** `compile_analog_block` is implemented and tested
(`piperine-lang/tests/jit_resistor.rs` — resistor + diode with exp, 5/5 pass).
Digital codegen and `FrontendLower` are next.

---

---

## 4. Codegen Implementation (`piperine-lang/src/codegen/`)

Implemented. File layout:

```
piperine-lang/src/codegen/
  mod.rs        — CodegenError, public re-exports
  analog.rs     — compile_analog_block() → JitAnalogDevice
                  CompiledAnalogBlock (residual + jacobian fn ptrs)
                  libm wrappers (sin, cos, exp, … registered with JITBuilder)
  expr.rs       — ExprCtx, emit_expr() — IrExpr → Cranelift Value
  autodiff.rs   — diff(expr, wrt) → IrExpr  (symbolic, with constant folding)
```

### Key API

```rust
// piperine-lang/src/codegen/analog.rs

pub fn compile_analog_block(
    name: &str,
    block: &IrAnalogBlock,
    terminals: &[PortBinding],
    param_names: &[String],
) -> Result<JitAnalogDevice, CodegenError>

pub struct JitAnalogDevice {
    pub name: String,
    pub param_names: Vec<String>,
    pub compiled: CompiledAnalogBlock,
}
impl JitAnalogDevice {
    pub fn eval_residual(&self, node_voltages: &[f64], params: &[f64], rhs: &mut [f64]);
    pub fn eval_jacobian(&self, node_voltages: &[f64], params: &[f64], jac: &mut [f64]);
}
```

### Autodiff strategy

**Symbolic diff over `IrExpr`** (Option D2 from the previous discussion).
`diff(expr, "branch_name")` walks the expression tree, applies chain rule,
returns a new `IrExpr`. The result is fed back through `emit_expr` to JIT-compile
the Jacobian entries — no dual-number overhead, no extra eval pass.

Implemented rules: Add, Sub, Mul (product rule), Div (quotient rule),
Exp, Ln, Log10, Sqrt, Sin, Cos, Tan, Asin, Acos, Atan, Abs, Pow, LimExp,
Select (differentiates both branches). Constant folding eliminates trivial
`x * 0`, `x * 1`, `x + 0` cases.

### Not yet implemented

- `DigitalDevice` codegen (always/initial blocks → Cranelift)
- `FrontendLower`: `Document` AST → `IrDesign`
- Voltage-contribution stamping (`V(branch) <+ expr`)
- Procedural `if/else` with actual Cranelift basic blocks (currently flattened)

### `mod.rs` — trait + entry point

```rust
// piperine-lang/src/codegen/mod.rs

use crate::ir::{AnalogIrInstance, DigitalIrInstance, IrDesign};
use proc_macro2::TokenStream;
use std::path::PathBuf;

pub mod analog;
pub mod autodiff;
pub mod compile;
pub mod digital;
pub mod expr;

#[derive(Debug)]
pub enum CodegenError {
    IoError(std::io::Error),
    CompileError { stdout: String, stderr: String },
}

/// Compile every Source instance in `design` to native .so files.
/// Returns paths to all compiled libraries for dlopen.
pub fn compile_design(design: &IrDesign, out_dir: &std::path::Path) -> Result<Vec<PathBuf>, CodegenError> {
    let mut libs = Vec::new();
    for inst in &design.analog_instances {
        if let crate::ir::AnalogBody::Source(block) = &inst.body {
            let ts = analog::emit_analog_device(&inst.model_name, block, &inst.terminals);
            let so = compile::emit_and_compile(&inst.model_name, ts, out_dir)?;
            libs.push(so);
        }
    }
    for inst in &design.digital_instances {
        if let crate::ir::DigitalBody::Source(block) = &inst.body {
            let ts = digital::emit_digital_device(&inst.model_name, block);
            let so = compile::emit_and_compile(&inst.model_name, ts, out_dir)?;
            libs.push(so);
        }
    }
    Ok(libs)
}
```

### `analog.rs` — `IrAnalogBlock` → `AnalogDevice` impl

```rust
// piperine-lang/src/codegen/analog.rs

use proc_macro2::TokenStream;
use quote::quote;
use crate::ir::instance::{IrAnalogBlock, IrAnalogStmt, IrContribution};
use super::expr;
use super::autodiff;

pub fn emit_analog_device(
    model_name: &str,
    block: &IrAnalogBlock,
    terminals: &[crate::ir::types::PortBinding],
) -> TokenStream {
    let struct_name = quote::format_ident!("{}", to_pascal(model_name));
    let num_terminals = terminals.len();
    let eval_body    = emit_eval_body(block);
    let resist_body  = emit_residual_resist(block);
    let jacobian_body = emit_jacobian_resist(block);

    quote! {
        use piperine_solver::analog::device::*;

        pub struct #struct_name;

        impl AnalogDevice for #struct_name {
            type ModelData    = ();
            type InstanceData = InstanceState;

            fn name(&self) -> &str { #model_name }
            fn num_terminals(&self) -> usize { #num_terminals }
            // ... other metadata ...

            fn eval(&self, _model: &(), inst: &mut InstanceState, sim: &mut SimInfo) -> EvalFlags {
                #eval_body
                EvalFlags::empty()
            }

            fn load_residual_resist(&self, _model: &(), inst: &InstanceState, rhs: &mut [f64]) {
                #resist_body
            }

            fn load_jacobian_resist(&self, _model: &(), inst: &InstanceState, jac: &mut [f64]) {
                #jacobian_body
            }

            // reactive / noise / spice_rhs follow same pattern ...
        }
    }
}

fn emit_eval_body(block: &IrAnalogBlock) -> TokenStream {
    let stmts: Vec<TokenStream> = block.statements.iter()
        .filter_map(|s| emit_analog_stmt_eval(s))
        .collect();
    quote! { #(#stmts)* }
}

fn emit_residual_resist(block: &IrAnalogBlock) -> TokenStream {
    // collect Contribution::Current stmts → rhs[node_idx] += expr
    let stamps: Vec<TokenStream> = block.statements.iter()
        .filter_map(|s| {
            if let IrAnalogStmt::Contribution(IrContribution::Current { branch, expr }) = s {
                let v = expr::emit_expr(expr);
                // node indices from branch mapping (TBD at lowering)
                Some(quote! { rhs[inst.#branch.pos] += #v; rhs[inst.#branch.neg] -= #v; })
            } else { None }
        })
        .collect();
    quote! { #(#stamps)* }
}

fn emit_jacobian_resist(block: &IrAnalogBlock) -> TokenStream {
    // For each contribution, differentiate wrt each node voltage via autodiff
    // TBD: autodiff::differentiate(expr, wrt_var) → IrExpr → emit
    quote! { /* TODO: autodiff pass */ }
}

fn emit_analog_stmt_eval(stmt: &IrAnalogStmt) -> Option<TokenStream> {
    match stmt {
        IrAnalogStmt::Assign { var, expr } => {
            let ident = quote::format_ident!("{}", var);
            let val = expr::emit_expr(expr);
            Some(quote! { let #ident = #val; })
        }
        IrAnalogStmt::IfElse { cond, then_, else_ } => {
            let c  = expr::emit_expr(cond);
            let t: Vec<_> = then_.iter().filter_map(emit_analog_stmt_eval).collect();
            let e: Vec<_> = else_.iter().filter_map(emit_analog_stmt_eval).collect();
            Some(quote! { if #c != 0.0 { #(#t)* } else { #(#e)* } })
        }
        // Contributions handled in residual/jacobian passes, not eval
        IrAnalogStmt::Contribution(_) | IrAnalogStmt::IndirectContribution(_) => None,
        IrAnalogStmt::Display { .. } => None, // skip in production eval
        IrAnalogStmt::LocalVar { name, init, .. } => {
            let id = quote::format_ident!("{}", name);
            let v  = init.as_ref().map(expr::emit_expr).unwrap_or(quote! { 0.0_f64 });
            Some(quote! { let mut #id = #v; })
        }
    }
}

fn to_pascal(s: &str) -> String {
    s.split('_').map(|w| {
        let mut c = w.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>()).unwrap_or_default() + c.as_str()
    }).collect()
}
```

### `expr.rs` — `IrExpr` → `TokenStream`

```rust
// piperine-lang/src/codegen/expr.rs

use proc_macro2::TokenStream;
use quote::quote;
use crate::ir::expr::{IrExpr, IrAnalogFn, IrBinOp, IrUnOp};

pub fn emit_expr(e: &IrExpr) -> TokenStream {
    match e {
        IrExpr::Literal(v)         => quote! { #v_f64 },
        IrExpr::Var(name)          => { let id = quote::format_ident!("{}", name); quote! { #id } }
        IrExpr::BranchVoltage(b)   => { let id = quote::format_ident!("v_{}", b); quote! { inst.#id } }
        IrExpr::BranchCurrent(b)   => { let id = quote::format_ident!("i_{}", b); quote! { inst.#id } }
        IrExpr::NodeVoltage(nid)   => { let idx = nid.0; quote! { sim.prev_solve[#idx] } }
        IrExpr::BinaryOp { op, lhs, rhs } => {
            let l = emit_expr(lhs);
            let r = emit_expr(rhs);
            match op {
                IrBinOp::Add => quote! { (#l + #r) },
                IrBinOp::Sub => quote! { (#l - #r) },
                IrBinOp::Mul => quote! { (#l * #r) },
                IrBinOp::Div => quote! { (#l / #r) },
                IrBinOp::Pow => quote! { f64::powf(#l, #r) },
                IrBinOp::Eq  => quote! { if #l == #r { 1.0_f64 } else { 0.0_f64 } },
                IrBinOp::Lt  => quote! { if #l < #r  { 1.0_f64 } else { 0.0_f64 } },
                // ... remaining ops ...
                _ => quote! { todo!() },
            }
        }
        IrExpr::UnaryOp { op, operand } => {
            let o = emit_expr(operand);
            match op {
                IrUnOp::Neg    => quote! { -#o },
                IrUnOp::LogNot => quote! { if #o == 0.0 { 1.0_f64 } else { 0.0_f64 } },
                _               => quote! { todo!() },
            }
        }
        IrExpr::AnalogFn { func, args } => emit_analog_fn(func, args),
        IrExpr::Select { cond, then_, else_ } => {
            let c = emit_expr(cond);
            let t = emit_expr(then_);
            let e = emit_expr(else_);
            quote! { if #c != 0.0 { #t } else { #e } }
        }
    }
}

fn emit_analog_fn(func: &IrAnalogFn, args: &[IrExpr]) -> TokenStream {
    let a: Vec<_> = args.iter().map(emit_expr).collect();
    match func {
        IrAnalogFn::Sin  => quote! { f64::sin(#(#a),*)  },
        IrAnalogFn::Cos  => quote! { f64::cos(#(#a),*)  },
        IrAnalogFn::Exp  => quote! { f64::exp(#(#a),*)  },
        IrAnalogFn::Ln   => quote! { f64::ln(#(#a),*)   },
        IrAnalogFn::Log  => quote! { f64::log10(#(#a),*)},
        IrAnalogFn::Sqrt => quote! { f64::sqrt(#(#a),*) },
        IrAnalogFn::Abs  => quote! { f64::abs(#(#a),*)  },
        IrAnalogFn::Pow  => quote! { f64::powf(#(#a),*) },
        IrAnalogFn::Min  => quote! { f64::min(#(#a),*)  },
        IrAnalogFn::Max  => quote! { f64::max(#(#a),*)  },
        IrAnalogFn::LimExp => quote! { piperine_solver::math::lim_exp(#(#a),*) },
        // Ddt → references state variable slot (allocated during lowering)
        IrAnalogFn::Ddt  => quote! { inst.ddt_state },
        // WhiteNoise / FlickerNoise → routed to load_noise, not eval; emit 0.0 here
        IrAnalogFn::WhiteNoise | IrAnalogFn::FlickerNoise => quote! { 0.0_f64 },
        _ => quote! { todo!("unimplemented analog fn") },
    }
}
```

### `autodiff.rs` — forward-mode dual-number differentiation

```rust
// piperine-lang/src/codegen/autodiff.rs
//
// Differentiates an IrExpr with respect to a named variable (node voltage, etc.)
// Returns a new IrExpr representing d(expr)/d(var).
// Used to generate Jacobian entries without an external AD library.

use crate::ir::expr::{IrExpr, IrAnalogFn, IrBinOp, IrUnOp};

/// Returns d(expr)/d(wrt) as a new IrExpr.
pub fn diff(expr: &IrExpr, wrt: &str) -> IrExpr {
    match expr {
        IrExpr::Literal(_)       => IrExpr::Literal(0.0),
        IrExpr::Var(v)           => if v == wrt { IrExpr::Literal(1.0) } else { IrExpr::Literal(0.0) },
        IrExpr::BranchVoltage(b) => if b == wrt { IrExpr::Literal(1.0) } else { IrExpr::Literal(0.0) },
        IrExpr::BranchCurrent(b) => if b == wrt { IrExpr::Literal(1.0) } else { IrExpr::Literal(0.0) },
        IrExpr::NodeVoltage(_)   => IrExpr::Literal(0.0), // treat node voltages as constants wrt branch vars

        IrExpr::BinaryOp { op, lhs, rhs } => {
            let dl = diff(lhs, wrt);
            let dr = diff(rhs, wrt);
            match op {
                // d(u+v) = du + dv
                IrBinOp::Add => add(dl, dr),
                // d(u-v) = du - dv
                IrBinOp::Sub => sub(dl, dr),
                // d(u*v) = du*v + u*dv
                IrBinOp::Mul => add(mul(dl, *rhs.clone()), mul(*lhs.clone(), dr)),
                // d(u/v) = (du*v - u*dv) / v^2
                IrBinOp::Div => div(
                    sub(mul(dl, *rhs.clone()), mul(*lhs.clone(), dr)),
                    mul(*rhs.clone(), *rhs.clone()),
                ),
                _ => IrExpr::Literal(0.0), // comparisons etc. → zero derivative
            }
        }
        IrExpr::UnaryOp { op, operand } => {
            let d = diff(operand, wrt);
            match op {
                IrUnOp::Neg => neg(d),
                _            => IrExpr::Literal(0.0),
            }
        }
        IrExpr::AnalogFn { func, args } => diff_fn(func, args, wrt),
        IrExpr::Select { cond, then_, else_ } => IrExpr::Select {
            cond: cond.clone(),
            then_: Box::new(diff(then_, wrt)),
            else_: Box::new(diff(else_, wrt)),
        },
    }
}

fn diff_fn(func: &IrAnalogFn, args: &[IrExpr], wrt: &str) -> IrExpr {
    // Chain rule: d f(u) = f'(u) * du
    let u  = &args[0];
    let du = diff(u, wrt);
    let fprime: IrExpr = match func {
        // d(exp(u)) = exp(u)
        IrAnalogFn::Exp  => call1(IrAnalogFn::Exp, u.clone()),
        // d(ln(u))  = 1/u
        IrAnalogFn::Ln   => div(IrExpr::Literal(1.0), u.clone()),
        // d(sqrt(u)) = 1/(2*sqrt(u))
        IrAnalogFn::Sqrt => div(IrExpr::Literal(1.0), mul(IrExpr::Literal(2.0), call1(IrAnalogFn::Sqrt, u.clone()))),
        // d(sin(u))  = cos(u)
        IrAnalogFn::Sin  => call1(IrAnalogFn::Cos, u.clone()),
        // d(cos(u))  = -sin(u)
        IrAnalogFn::Cos  => neg(call1(IrAnalogFn::Sin, u.clone())),
        _                 => IrExpr::Literal(0.0), // conservative: treat unknown fns as constant
    };
    mul(fprime, du)
}

// ── helpers ────────────────────────────────────────────────────────────────────

fn add(a: IrExpr, b: IrExpr) -> IrExpr {
    IrExpr::BinaryOp { op: IrBinOp::Add, lhs: Box::new(a), rhs: Box::new(b) }
}
fn sub(a: IrExpr, b: IrExpr) -> IrExpr {
    IrExpr::BinaryOp { op: IrBinOp::Sub, lhs: Box::new(a), rhs: Box::new(b) }
}
fn mul(a: IrExpr, b: IrExpr) -> IrExpr {
    IrExpr::BinaryOp { op: IrBinOp::Mul, lhs: Box::new(a), rhs: Box::new(b) }
}
fn div(a: IrExpr, b: IrExpr) -> IrExpr {
    IrExpr::BinaryOp { op: IrBinOp::Div, lhs: Box::new(a), rhs: Box::new(b) }
}
fn neg(a: IrExpr) -> IrExpr {
    IrExpr::UnaryOp { op: IrUnOp::Neg, operand: Box::new(a) }
}
fn call1(f: IrAnalogFn, a: IrExpr) -> IrExpr {
    IrExpr::AnalogFn { func: f, args: vec![a] }
}
```

### `compile.rs` — write + invoke rustc

```rust
// piperine-lang/src/codegen/compile.rs

use proc_macro2::TokenStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use super::CodegenError;

/// Write `ts` to `<out_dir>/<name>.rs`, compile to `<out_dir>/lib<name>.so`.
/// Returns the path to the compiled shared library.
pub fn emit_and_compile(
    name: &str,
    ts: TokenStream,
    out_dir: &Path,
) -> Result<PathBuf, CodegenError> {
    let src_path = out_dir.join(format!("{name}.rs"));
    let so_path  = out_dir.join(format!("lib{name}.so"));

    // Write generated source
    let src = prettyplease::unparse(
        &syn::parse2::<syn::File>(ts).expect("generated TokenStream is valid Rust")
    );
    std::fs::write(&src_path, src).map_err(CodegenError::IoError)?;

    // Compile
    let status = Command::new("rustc")
        .args([
            "--edition=2021",
            "--crate-type=cdylib",
            "-L", "target/debug/deps",   // find piperine-solver etc.
            "--extern", "piperine_solver=...",
            "-o", so_path.to_str().unwrap(),
            src_path.to_str().unwrap(),
        ])
        .status()
        .map_err(CodegenError::IoError)?;

    if status.success() {
        Ok(so_path)
    } else {
        Err(CodegenError::CompileError {
            stdout: String::new(),
            stderr: format!("rustc exited with {status}"),
        })
    }
}
```

---

## 5. Key Traits (targets for codegen)

### `AnalogDevice` — `piperine-solver/src/analog/device.rs`

```rust
pub trait AnalogDevice: Send + Sync {
    type ModelData: Send + Sync;
    type InstanceData: Send + Sync;

    fn name(&self) -> &str;
    fn num_nodes(&self) -> usize;
    fn num_terminals(&self) -> usize;
    fn num_states(&self) -> usize;
    fn instance_size(&self) -> usize;
    fn num_resistive_jacobian_entries(&self) -> usize;
    fn num_reactive_jacobian_entries(&self) -> usize;

    fn setup_model(&self, model: &mut Self::ModelData, paras: &SimParams, info: &mut InitInfo);
    fn setup_instance(&self, model: &Self::ModelData, instance: &mut Self::InstanceData,
                      temp: f64, flags: SimFlags, paras: &SimParams, info: &mut InitInfo);
    fn allocate_nodes(&self, instance_name: &str,
                      terminals: &[NodeIdentifier], netlist: &mut Netlist) -> Vec<Option<AnalogReference>>;
    fn bind_nodes(&self, instance: &mut Self::InstanceData, node_refs: &mut Vec<Option<AnalogReference>>);
    fn set_params(&self, model: &mut Self::ModelData, instance: &mut Self::InstanceData,
                  params: &[(String, f64)], str_params: &[(String, String)]);

    fn eval(&self, model: &Self::ModelData, instance: &mut Self::InstanceData,
            sim_info: &mut SimInfo) -> EvalFlags;
    fn bound_step_hint(&self, instance: &Self::InstanceData) -> f64;
    fn read_opvars(&self, model: &Self::ModelData, instance: &Self::InstanceData) -> Vec<(String, f64)>;

    fn load_residual_resist(&self, model: &Self::ModelData, instance: &Self::InstanceData, rhs: &mut [f64]);
    fn load_residual_react(&self, model: &Self::ModelData, instance: &Self::InstanceData, rhs: &mut [f64]);
    fn load_jacobian_resist(&self, model: &Self::ModelData, instance: &Self::InstanceData, jacobian: &mut [f64]);
    fn load_jacobian_react(&self, model: &Self::ModelData, instance: &Self::InstanceData,
                           step: f64, jacobian: &mut [f64]);
    fn load_spice_rhs_dc(&self, model: &Self::ModelData, instance: &Self::InstanceData,
                         rhs: &mut [f64], prev_solve: &[f64]);
    fn load_spice_rhs_tran(&self, model: &Self::ModelData, instance: &Self::InstanceData,
                           rhs: &mut [f64], prev_solve: &[f64], alpha: f64);

    fn num_noise_sources(&self) -> usize { 0 }
    fn noise_source_node_pairs(&self) -> Vec<(usize, usize)> { vec![] }
    fn load_noise(&self, _model: &Self::ModelData, _instance: &Self::InstanceData,
                  _freq: f64, _noise_rhs: &mut [f64]) {}
}
```

### `DigitalDevice` — `piperine-solver/src/digital/state.rs`

```rust
pub trait DigitalDevice {
    fn has_input_on(&self, changed_nets: &HashSet<DigitalNet>) -> bool;
    fn eval(&mut self, current_time: f64, nets: &[LogicValue],
            event_queue: &mut BinaryHeap<Reverse<DigitalEvent>>);
    fn input_nets(&self) -> &[DigitalNet] { &[] }
    fn output_nets(&self) -> &[DigitalNet] { &[] }
}
```

---

## 6. IR Data Model (`piperine-lang/src/ir/`)

The IR is flat and fully elaborated: no hierarchy, no unresolved names, no
symbolic parameter references. Everything the codegen pass needs is present and
typed.

### 6.1 `IrDesign` — top-level container

```rust
pub struct IrDesign {
    pub meta: IrMeta,
    pub disciplines: HashMap<String, IrDiscipline>,
    pub natures:     HashMap<String, IrNature>,
    pub nets:        HashMap<NodeId, IrNet>,
    pub analog_instances:  Vec<AnalogIrInstance>,
    pub digital_instances: Vec<DigitalIrInstance>,
    pub connect_instances: Vec<ConnectIrInstance>,
}
```

### 6.2 `AnalogIrInstance`

```rust
pub enum AnalogBody {
    /// IR source for codegen — compiled to Rust implementing `AnalogDevice`.
    Source(IrAnalogBlock),
    /// Pre-compiled OSDI .so — loaded at runtime, no codegen needed.
    Osdi { path: PathBuf },
    /// Built-in SPICE primitive — solver has hardcoded stamp formula.
    Primitive,
}
```

### 6.3 `IrAnalogBlock` → codegen mapping

| IR construct | Generated method |
|---|---|
| `IrContribution::Current { branch, expr }` | `load_residual_resist` + `load_jacobian_resist` (via `autodiff::diff`) |
| `IrContribution::Voltage { branch, expr }` | KVL stamp in `load_residual_resist` / `load_jacobian_resist` |
| `IrIndirectContribution` | Companion model equation in `eval` |
| `IrAnalogStmt::Assign` | Assignment in `eval` |
| `IrAnalogStmt::LocalVar` | Local variable declaration in `eval` |
| `IrAnalogStmt::IfElse` | `if` block in `eval` |
| `IrAnalogFn::WhiteNoise / FlickerNoise` | `load_noise` |
| `IrAnalogFn::Ddt` | State variable slot + `load_residual_react` / `load_jacobian_react` |

### 6.4 `IrExpr` — expression language

```rust
pub enum IrExpr {
    Literal(f64),
    Var(String),               // local var or parameter
    BranchVoltage(String),     // V(branch)
    BranchCurrent(String),     // I(branch)
    NodeVoltage(NodeId),       // V(node) relative to GND
    AnalogFn { func: IrAnalogFn, args: Vec<IrExpr> },
    BinaryOp { op: IrBinOp, lhs: Box<IrExpr>, rhs: Box<IrExpr> },
    UnaryOp  { op: IrUnOp,  operand: Box<IrExpr> },
    Select   { cond: Box<IrExpr>, then_: Box<IrExpr>, else_: Box<IrExpr> },
}
```

### 6.5 `DigitalBody::Source` → codegen mapping

| IR construct | Generated method |
|---|---|
| `IrAlwaysBlock { sensitivity: Posedge(net), body }` | Rising-edge detection in `eval` |
| `IrAlwaysBlock { sensitivity: Star, body }` | Combinational eval — always re-evaluate |
| `IrStmt::NonBlockingAssign` | Push `DigitalEvent` to `event_queue` |
| `IrStmt::Assign` | Immediate assignment inside `eval` |
| `input_ports / output_ports` | `input_nets()` / `output_nets()` return values |

### 6.6 `ConnectIrInstance` — A2D / D2A bridges

```rust
pub enum ConnectKind {
    A2D    { threshold: f64, hysteresis: f64 },
    D2A    { v_high: f64, v_low: f64, rise_time: f64 },
    Custom { model_name: String, parameters: HashMap<String, f64> },
}
```

---

## 7. `FrontendLower` Trait

```rust
// piperine-lang/src/lowering/mod.rs
pub trait FrontendLower {
    type Ast;
    type Error;
    fn lower(ast: Self::Ast) -> Result<IrDesign, Self::Error>;
}
```

`piperine-parser` will implement `FrontendLower` for `Document`. This is the
next major implementation step after BNF P1 gaps are closed.

---

## 8. Roadmap

- [x] Parser — BNF ~80% covered (gaps in `docs/BNF-AMS.md`)
- [x] `IrDesign` data model (`piperine-lang/src/ir/`)
- [x] `AnalogDevice` / `DigitalDevice` traits (`piperine-solver`)
- [x] OSDI loader for external models (`piperine-solver/src/analog/osdi/`)
- [x] Digital DAG scheduler with back-edge detection (`piperine-solver/src/digital/`)
- [ ] BNF P1 gaps: `task`, `#(param)` module header, `specify`/`specparam` dispatch, gate routing
- [ ] `FrontendLower` impl — `Document` → `IrDesign` (`piperine-lang/src/lowering/`)
- [ ] Codegen skeleton — `piperine-lang/src/codegen/` (expr.rs, analog.rs, autodiff.rs, compile.rs)
- [ ] End-to-end test: simple resistor VA → `.so` → DC solve
- [ ] Filter functions (`ddt`, `idt`) — state variable allocation in lowering
- [ ] Full autodiff coverage (all `IrAnalogFn` variants)
- [ ] Digital codegen (`IrDigitalBlock` → `DigitalDevice`)

---

## 9. Testbench Language (future)

A `.ptb` format is planned to fill the testbench gap in Verilog-AMS. It would
lower to the same `IrDesign` via its own `FrontendLower` impl. Not yet designed
or prioritized.

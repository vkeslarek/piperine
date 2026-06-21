# Piperine — OpenVAF-Reloaded Integration (Phase 2)

**Purpose:** step-by-step guide for compiling Verilog-A modules to OSDI automatically.
Any module with an `analog` block is compiled transparently by the `openvaf` Rust library
— no user-facing task, no subprocess, no external binary required.

---

## 0. Target program and expected output

```verilog
// examples/diode_op.ppr

// VA module — has analog block → compiled automatically by OpenVAF
module simple_diode(anode, cathode);
  inout anode, cathode;
  electrical anode, cathode;

  parameter real is = 1e-14 from (0:inf);

  analog begin
    I(anode, cathode) <+ is * (exp(V(anode, cathode) / 0.02585) - 1.0);
  end
endmodule

// Testbench — purely declarative + analysis calls, zero OSDI plumbing
module tb;
  extern module spice_vsource(inout p, inout n; parameter real val = 0.0);

  simple_diode #(.is(1e-14)) D1 (.anode(vout), .cathode(gnd));
  spice_vsource #(.val(0.7))  V1 (.p(vout),  .n(gnd));

  initial begin
    $op();
    $display("Id = %g A", $I("d1"));
  end
endmodule
```

Expected output (approximate):
```
Id = 0.026752 A
```

**Rule: any module with an `analog` block → OpenVAF backend. No user action required.**

Full pipeline:
```
parse .ppr
  → find modules with analog blocks (VA modules)
  → compile each with openvaf Rust lib → .osdi (cached by mtime)
  → pre_osdi each .osdi into ngspice
  → register OsdiHardwareDefinition for each VA module
  → elaborate testbench (VA instances now resolvable)
  → load netlist into ngspice
  → interpret initial block
```

---

## 1. Integration approach — Rust library linkage

OpenVAF-Reloaded is GPL-3.0. Piperine links it as a Rust library (git submodule at
`tools/OpenVAF-Reloaded`). The `piperine-openvaf` crate depends directly on `openvaf`
and calls `openvaf::compile()` in-process.

Consequences:
- No external binary needed — `openvaf-r` does NOT need to be installed
- Piperine itself becomes GPL-3.0 (planned: open source)
- LLVM 18.1.x and `libpolly-18-dev` required **at build time**

---

## 2. Build prerequisites

### 2.1 System packages

Use `scripts/setup-dev.sh` (installs everything automatically):

```bash
bash scripts/setup-dev.sh
```

Or manually on Debian/Ubuntu:

```bash
sudo apt install \
    llvm-18 llvm-18-dev \
    clang-18 libclang-18-dev \
    lld-18 \
    libpolly-18-dev \
    libngspice0 libngspice0-dev
```

### 2.2 Environment variable

```bash
export LLVM_SYS_181_PREFIX=$(llvm-config-18 --prefix)
```

Add to `~/.bashrc` or `~/.zshrc` to persist across sessions.

### 2.3 Verify

```bash
llvm-config-18 --version   # must print 18.1.x
cargo check -p piperine-openvaf
```

---

## 3. Git submodule — `tools/OpenVAF-Reloaded`

OpenVAF-Reloaded is pinned as a git submodule at a known-good commit.

```bash
# Already done — submodule is at tools/OpenVAF-Reloaded
git submodule update --init --recursive
```

The submodule tracks commit `d878f55` ("change versions") from
`https://github.com/OpenVAF/OpenVAF-Reloaded`.

To update to a newer commit:

```bash
cd tools/OpenVAF-Reloaded
git fetch origin
git checkout <new-commit>
cd ../..
git add tools/OpenVAF-Reloaded
git commit -m "bump OpenVAF-Reloaded to <new-commit>"
```

---

## 4. Salsa patch in root `Cargo.toml`

OpenVAF-Reloaded uses a custom fork of `salsa` (adds `salsa::Cycle`). Without the
`[patch]` entry, Cargo resolves `salsa 0.17.0-pre.2` from crates.io which is missing
that type and fails to compile.

The patch is already in `Cargo.toml`:

```toml
[patch.crates-io]
salsa = { git = "https://github.com/pascalkuthe/salsa", rev = "73532d7d4d8b5b27f2c9f189a76e012d1fc4de09" }
```

---

## 5. New crate: `piperine-openvaf`

```
crates/piperine-openvaf/
  Cargo.toml
  build.rs           ← LLVM 18 version check with clear error message
  src/
    lib.rs           ← OpenVafPlugin + compile_va() helper
    compiler.rs      ← LibraryCompiler (implements AnalogCompilerBackend)
    cache.rs         ← mtime-based cache lookup
    osdi_hardware.rs ← OsdiHardwareDefinition (implements HardwareDefinition)
```

No system task file — OSDI compilation is automatic, not user-triggered.

### 5.1 `crates/piperine-openvaf/Cargo.toml`

```toml
[package]
name    = "piperine-openvaf"
version = "0.1.0"
edition = "2021"

[dependencies]
piperine-circuit     = { path = "../piperine-circuit" }
piperine-interpreter = { path = "../piperine-interpreter" }

# OpenVAF — linked directly as a Rust library.
# Requires LLVM 18 + clang at build time (see build.rs).
# License: GPL-3.0 — this crate and the piperine binary are GPL as a result.
openvaf = { path = "../../tools/OpenVAF-Reloaded/openvaf/openvaf" }

camino = "1.1.4"
dirs   = "5"

[build-dependencies]
llvm-sys = "181.1.1"
```

### 5.2 `build.rs`

Checks LLVM 18 is present and emits a clear error if not. Reads `LLVM_SYS_181_PREFIX`
or falls back to `llvm-config-18` / `llvm-config` on `$PATH`. Bypass with
`PIPERINE_OPENVAF_SKIP_LLVM_CHECK=1`.

---

## 6. `compiler.rs` — LibraryCompiler

Calls `openvaf::compile()` directly in-process. No subprocess, no external binary.

**File: `crates/piperine-openvaf/src/compiler.rs`**

```rust
use std::path::{Path, PathBuf};

use camino::Utf8PathBuf;
use openvaf::{
    compile, host_triple, CompilationDestination, CompilationTermination,
    LLVMCodeGenOptLevel, Opts, Target,
};
use piperine_interpreter::{AnalogCompilerBackend, InterpreterError, SimulatorBackend};

pub struct LibraryCompiler;

impl AnalogCompilerBackend for LibraryCompiler {
    fn name(&self) -> &str { "openvaf" }

    fn compile(
        &self,
        source_path: &Path,
        output_directory: &Path,
    ) -> Result<PathBuf, InterpreterError> {
        let input = Utf8PathBuf::from_path_buf(source_path.to_path_buf())
            .map_err(|p| err(format!("source path not UTF-8: {}", p.display())))?;

        let stem = input.file_stem().unwrap_or("module");
        let lib_file = Utf8PathBuf::from_path_buf(output_directory.join(format!("{stem}.osdi")))
            .map_err(|p| err(format!("output path not UTF-8: {}", p.display())))?;

        let host = host_triple();
        let target = Target::search(host)
            .ok_or_else(|| err(format!("host triple '{host}' not supported")))?;

        let opts = Opts {
            input,
            output: CompilationDestination::Path { lib_file: lib_file.clone() },
            defines: Vec::new(),
            lints: Vec::new(),
            codegen_opts: Vec::new(),
            include: Vec::new(),
            opt_lvl: LLVMCodeGenOptLevel::LLVMCodeGenLevelDefault,
            target,
            target_cpu: "native".to_string(),
            dry_run: false,
            dump_mir: false, dump_unopt_mir: false,
            dump_ir: false,  dump_unopt_ir: false,
        };

        match compile(&opts).map_err(|e| err(format!("openvaf compile: {e}")))? {
            CompilationTermination::Compiled { lib_file: out } => Ok(out.into_std_path_buf()),
            CompilationTermination::FatalDiagnostic =>
                Err(err("compilation failed with fatal diagnostic (see stderr)")),
        }
    }

    fn pre_load(
        &self,
        artifact_path: &Path,
        simulator: &mut dyn SimulatorBackend,
    ) -> Result<(), InterpreterError> {
        let path_str = artifact_path.to_str()
            .ok_or_else(|| err(format!("artifact path not UTF-8: {}", artifact_path.display())))?;
        simulator.run_command(&format!("pre_osdi {path_str}"))
    }
}

fn err(msg: impl Into<String>) -> InterpreterError {
    InterpreterError::SimulatorError(msg.into())
}
```

---

## 7. `cache.rs`

Mtime-based cache. Filename: `<stem>-<mtime_secs>.osdi`. Cheap, avoids md5 dep.

**File: `crates/piperine-openvaf/src/cache.rs`**

```rust
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Returns cached `.osdi` path if it exists and source hasn't changed.
pub fn lookup(source_path: &Path, cache_dir: &Path) -> Option<PathBuf> {
    let mtime = mtime_secs(source_path)?;
    let stem = source_path.file_stem()?.to_str()?;
    let candidate = cache_dir.join(format!("{stem}-{mtime}.osdi"));
    if candidate.exists() { Some(candidate) } else { None }
}

/// Returns the output path to write into (creates cache dir if needed).
pub fn output_path(source_path: &Path, cache_dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(cache_dir)?;
    let mtime = mtime_secs(source_path).unwrap_or(0);
    let stem = source_path.file_stem().and_then(|s| s.to_str()).unwrap_or("module");
    Ok(cache_dir.join(format!("{stem}-{mtime}.osdi")))
}

fn mtime_secs(path: &Path) -> Option<u64> {
    path.metadata().ok()?.modified().ok()?
        .duration_since(SystemTime::UNIX_EPOCH).ok()
        .map(|d| d.as_secs())
}
```

---

## 8. `osdi_hardware.rs` — OsdiHardwareDefinition

The SPICE line for an OSDI device uses ngspice's `N`-prefix syntax:
```
N<name> <node1> <node2> ... <modelname> [param=value ...]
```

`<modelname>` is the VA module name exactly as declared (`simple_diode`). ngspice
registers it automatically when `pre_osdi` is called. Port order matches the VA
`inout` declaration order.

**File: `crates/piperine-openvaf/src/osdi_hardware.rs`**

```rust
use piperine_circuit::{
    ConnectionMap, ElaborationError, HardwareDefinition, HardwareInstance,
    ParameterDefinition, ParameterMap, PortDefinition,
};

#[derive(Debug)]
pub struct OsdiHardwareDefinition {
    /// VA module name — becomes the ngspice model name after `pre_osdi`.
    pub module_name: String,
    /// Port names in declaration order (matches the VA `inout` list).
    pub port_names: Vec<String>,
    /// Parameter definitions with defaults extracted from the parsed VA source.
    pub parameter_definitions: Vec<ParameterDefinition>,
}

impl HardwareDefinition for OsdiHardwareDefinition {
    fn name(&self) -> &str { &self.module_name }
    fn ports(&self) -> &[PortDefinition] { &[] }
    fn parameters(&self) -> &[ParameterDefinition] { &self.parameter_definitions }

    fn instantiate(
        &self,
        instance_name: &str,
        parameters: &ParameterMap,
        connections: &ConnectionMap,
    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {
        let nets: Vec<String> = self.port_names
            .iter()
            .map(|port| {
                connections.get(port).cloned().ok_or_else(|| {
                    ElaborationError::ConnectionError {
                        instance: instance_name.to_string(),
                        detail: format!("port `{port}` not connected"),
                    }
                })
            })
            .collect::<Result<_, _>>()?;

        Ok(Box::new(OsdiInstance {
            instance_name: instance_name.to_string(),
            model_name: self.module_name.clone(),
            nets,
            parameters: parameters.clone(),
        }))
    }
}

#[derive(Debug)]
struct OsdiInstance {
    instance_name: String,
    model_name: String,
    nets: Vec<String>,
    parameters: ParameterMap,
}

impl HardwareInstance for OsdiInstance {
    fn instance_name(&self) -> &str { &self.instance_name }

    fn spice_lines(&self) -> Vec<String> {
        // N<name> <node1> ... <nodeN> <modelname> [key=val ...]
        let mut parts = vec![format!("N{}", self.instance_name)];
        parts.extend(self.nets.clone());
        parts.push(self.model_name.clone());
        for (key, val) in &self.parameters {
            parts.push(format!("{key}={val}"));
        }
        vec![parts.join(" ")]
    }
}
```

---

## 9. `lib.rs` — OpenVafPlugin

```rust
mod cache;
mod compiler;

pub use compiler::LibraryCompiler;

use piperine_interpreter::{AnalogCompilerBackend, Plugin};

pub struct OpenVafPlugin {
    pub cache_dir: Option<std::path::PathBuf>,
}

impl OpenVafPlugin {
    pub fn new() -> Self { Self { cache_dir: None } }

    pub fn with_cache_dir(cache_dir: std::path::PathBuf) -> Self {
        Self { cache_dir: Some(cache_dir) }
    }
}

impl Default for OpenVafPlugin {
    fn default() -> Self { Self::new() }
}

impl Plugin for OpenVafPlugin {
    fn name(&self) -> &str { "openvaf" }

    fn analog_compiler(&self) -> Option<Box<dyn AnalogCompilerBackend>> {
        Some(Box::new(LibraryCompiler))
    }
}

/// Compile a VA source file with mtime-based caching.
/// Returns path to `.osdi` artifact ready to pass to `pre_osdi`.
pub fn compile_va(
    source_path: &std::path::Path,
    cache_dir: &std::path::Path,
) -> Result<std::path::PathBuf, piperine_interpreter::InterpreterError> {
    if let Some(cached) = cache::lookup(source_path, cache_dir) {
        return Ok(cached);
    }
    let output = cache::output_path(source_path, cache_dir).map_err(|e| {
        piperine_interpreter::InterpreterError::SimulatorError(format!("cache dir: {e}"))
    })?;
    piperine_interpreter::AnalogCompilerBackend::compile(
        &LibraryCompiler,
        source_path,
        output.parent().unwrap_or(cache_dir),
    )
}

fn default_cache_dir() -> std::path::PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("piperine")
        .join("osdi")
}
```

---

## 10. Extensions to `piperine-circuit`

Two new public functions in `crates/piperine-circuit/src/elaboration.rs`.

### 10.1 `extract_va_modules`

```rust
pub struct VaModuleInfo {
    pub module_name: String,
    pub port_names: Vec<String>,
    /// (parameter_name, default_expr)
    pub parameter_defaults: Vec<(String, cvaf::ast::Expr)>,
}

/// Find all VA modules (analog block present, no initial block).
pub fn extract_va_modules(document: &Document) -> Vec<VaModuleInfo> {
    document.modules.iter()
        .filter(|m| !m.analog_blocks.is_empty() && m.initial_blocks.is_empty())
        .map(|m| VaModuleInfo {
            module_name: m.name.clone(),
            port_names: m.ports.iter().map(|p| p.name.clone()).collect(),
            parameter_defaults: m.parameters.iter()
                .filter(|p| !p.is_local)
                .map(|p| (p.name.clone(), p.default_value.clone()))
                .collect(),
        })
        .collect()
}
```

### 10.2 `eval_default_expr`

```rust
use cvaf::ast::{Expr, Literal, PrefixOp};

/// Convert a compile-time-constant AST expression to a ParameterValue.
pub fn eval_default_expr(expr: &Expr) -> Option<ParameterValue> {
    match expr {
        Expr::Literal(Literal::StdRealNumber(s)) =>
            s.parse::<f64>().ok().map(ParameterValue::Real),
        Expr::Literal(Literal::SiRealNumber(s)) =>
            parse_si_real(s).map(ParameterValue::Real),
        Expr::Literal(Literal::IntNumber(s)) =>
            s.parse::<i64>().ok().map(ParameterValue::Integer),
        Expr::Literal(Literal::StrLit(s)) =>
            Some(ParameterValue::String(s.clone())),
        Expr::Prefix(PrefixOp::Neg, inner) => match eval_default_expr(inner)? {
            ParameterValue::Real(v)    => Some(ParameterValue::Real(-v)),
            ParameterValue::Integer(v) => Some(ParameterValue::Integer(-v)),
            _ => None,
        },
        _ => None,
    }
}
```

### 10.3 Update `crates/piperine-circuit/src/lib.rs`

```rust
pub use elaboration::{
    elaborate, ElaborationResult,
    extract_va_modules, VaModuleInfo,
    eval_default_expr,
};
```

---

## 11. Root `Cargo.toml` — required entries

### Workspace members

```toml
[workspace]
members = [
    "crates/piperine-common",
    "crates/piperine-worker",
    "crates/piperine-coordinator",
    "crates/piperine-parser",
    "crates/piperine-circuit",
    "crates/piperine-interpreter",
    "crates/piperine-ngspice",
    "crates/piperine-openvaf",  # ← Phase 2
]
```

### Salsa patch (required — OpenVAF uses custom salsa fork)

```toml
[patch.crates-io]
salsa = { git = "https://github.com/pascalkuthe/salsa", rev = "73532d7d4d8b5b27f2c9f189a76e012d1fc4de09" }
```

### Root binary dependency (optional feature to allow building without LLVM 18)

```toml
piperine-openvaf = { path = "crates/piperine-openvaf", optional = true }
```

---

## 12. Updated `src/main.rs` — full Phase 2 flow

```rust
use std::path::PathBuf;

use piperine_circuit::{
    HardwareRegistry, ParameterDefinition,
    elaborate, extract_va_modules, eval_default_expr,
};
use piperine_interpreter::{Plugin, SystemTaskRegistry, Interpreter, Scope};
use piperine_ngspice::NgspicePlugin;
use piperine_openvaf::{OpenVafPlugin, OsdiHardwareDefinition, compile_va};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: piperine <file.ppr>");
        std::process::exit(1);
    }
    if let Err(error) = run(PathBuf::from(&args[1])) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run(path: PathBuf) -> Result<(), String> {
    // ── 1. Parse ─────────────────────────────────────────────────────────────
    let document = cvaf::parse_file(&path).map_err(|e| format!("parse: {e}"))?;

    // ── 2. Find VA modules (analog block, no initial block) ───────────────────
    let va_modules = extract_va_modules(&document);

    // ── 3. Build OsdiHardwareDefinitions from parsed metadata ─────────────────
    let osdi_defs: Vec<OsdiHardwareDefinition> = va_modules.iter()
        .map(|info| OsdiHardwareDefinition {
            module_name: info.module_name.clone(),
            port_names: info.port_names.clone(),
            parameter_definitions: info.parameter_defaults.iter()
                .map(|(name, expr)| ParameterDefinition {
                    name: name.clone(),
                    default: eval_default_expr(expr),
                })
                .collect(),
        })
        .collect();

    // ── 4. Register OSDI and ngspice hardware/tasks ───────────────────────────
    let mut hardware_registry = HardwareRegistry::new();
    for def in osdi_defs {
        hardware_registry.register(Box::new(def));
    }

    let mut task_registry = SystemTaskRegistry::new();
    let plugins: Vec<Box<dyn Plugin>> = vec![
        Box::new(NgspicePlugin::default()),
        Box::new(OpenVafPlugin::default()),
    ];

    let mut simulator_backend = None;
    for plugin in &plugins {
        plugin.register_hardware(&mut hardware_registry);
        plugin.register_tasks(&mut task_registry);
        if simulator_backend.is_none() {
            simulator_backend = plugin.simulator_backend();
        }
    }

    let mut simulator = simulator_backend
        .ok_or("no simulator backend — is piperine-ngspice registered?")?;

    // ── 5. Compile VA → OSDI, pre_osdi BEFORE load_circuit ───────────────────
    // ngspice must know OSDI models before parsing the netlist that uses them.
    if !va_modules.is_empty() {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("piperine/osdi");

        // One .ppr file → one .osdi (may contain multiple module descriptors).
        let osdi_path = compile_va(&path, &cache_dir)
            .map_err(|e| format!("openvaf: {e}"))?;

        simulator
            .run_command(&format!("pre_osdi {}", osdi_path.display()))
            .map_err(|e| format!("pre_osdi: {e}"))?;
    }

    // ── 6. Elaborate testbench ────────────────────────────────────────────────
    let mut elaboration = elaborate(&document, &hardware_registry)
        .map_err(|e| format!("elaboration: {e}"))?;
    elaboration.spice_lines.push(".end".to_string());

    // ── 7. Load netlist (OSDI models already registered in step 5) ────────────
    simulator
        .load_circuit(&elaboration.spice_lines)
        .map_err(|e| format!("circuit load: {e}"))?;

    // ── 8. Run interpreter ────────────────────────────────────────────────────
    let mut interpreter = Interpreter::new(simulator.as_mut(), &task_registry);
    let mut scope = Scope::default();
    interpreter
        .exec(&elaboration.initial_statement, &mut scope)
        .map_err(|e| format!("runtime: {e}"))?;

    Ok(())
}
```

**Critical ordering:** Step 5 (`pre_osdi`) MUST happen before Step 6 (`elaborate`) and
Step 7 (`load_circuit`). ngspice registers OSDI device models when `pre_osdi` is called;
it must see them before parsing any netlist line that references them.

---

## 13. How ngspice `pre_osdi` works

ngspice's `pre_osdi <path>` command:

1. `dlopen`s the `.osdi` shared library
2. Reads `OSDI_NUM_DESCRIPTORS` and `OSDI_DESCRIPTORS` exported symbols
3. For each descriptor, registers a device model named `descriptor.name`
4. After this, SPICE lines `N<name> <nodes> <modelname> [params]` are valid

The model name in the SPICE line must EXACTLY match the VA `module` name. For `simple_diode`:
```
ND1 vout 0 simple_diode is=1e-14
```

This is what `OsdiInstance::spice_lines()` generates (Section 8).

---

## 14. Dependency graph

```
piperine-common
    ↑
piperine-worker         piperine-parser
    ↑                       ↑
piperine-coordinator    piperine-circuit
                            ↑
                        piperine-interpreter
                            ↑               ↑
                        piperine-ngspice    piperine-openvaf ← openvaf (GPL)
                            ↑               ↑
                            piperine (binary)
```

`piperine-openvaf` does NOT depend on `piperine-ngspice`.
`openvaf` pulls in LLVM 18 at build time via `llvm-sys`.

---

## 15. OSDI 0.4 — what Piperine uses

Piperine does NOT implement an OSDI host. ngspice is the host. Piperine only:

1. Calls `pre_osdi <path>` to register the device
2. Emits `N`-prefix SPICE lines to instantiate it

For reference, the exported symbols OpenVAF puts in every `.osdi` file:

```c
uint32_t          OSDI_VERSION_MAJOR;   // 0
uint32_t          OSDI_VERSION_MINOR;   // 4
uint32_t          OSDI_NUM_DESCRIPTORS;
OsdiDescriptor   *OSDI_DESCRIPTORS;

typedef struct OsdiDescriptor {
    char            *name;          // VA module name → ngspice model name
    uint32_t         num_nodes;
    OsdiNode        *nodes;
    uint32_t         num_params;
    OsdiParamOpvar  *param_opvar;
    // + function pointers called by ngspice
} OsdiDescriptor;
```

---

## 16. Implementation checklist

- [x] **16.1** Git submodule `tools/OpenVAF-Reloaded` pinned to `d878f55`
- [x] **16.2** `[patch.crates-io]` salsa in root `Cargo.toml`
- [x] **16.3** `crates/piperine-openvaf/Cargo.toml` with `openvaf` path dep
- [x] **16.4** `crates/piperine-openvaf/build.rs` — LLVM 18 version check
- [x] **16.5** `crates/piperine-openvaf/src/compiler.rs` — `LibraryCompiler`
- [x] **16.6** `crates/piperine-openvaf/src/cache.rs` — mtime-based cache
- [x] **16.7** `crates/piperine-openvaf/src/lib.rs` — `OpenVafPlugin` + `compile_va`
- [x] **16.8** `crates/piperine-openvaf` added to workspace members and root deps
- [x] **16.9** `scripts/setup-dev.sh` — full dev environment setup script
- [x] **16.10** `cargo check -p piperine-openvaf` passes (verified with LLVM 18.1.8)

**Remaining:**

- [ ] **16.11** `crates/piperine-openvaf/src/osdi_hardware.rs` — `OsdiHardwareDefinition`
      Verify: `cargo check -p piperine-openvaf`

- [ ] **16.12** `eval_default_expr` in `piperine-circuit/src/elaboration.rs` (Section 10.2)
      Verify: `cargo check -p piperine-circuit`

- [ ] **16.13** `extract_va_modules` + `VaModuleInfo` in `piperine-circuit/src/elaboration.rs` (Section 10.1)
      Verify unit test: parse module with `analog begin ... end`, one entry returned

- [ ] **16.14** Update `piperine-circuit/src/lib.rs` re-exports (Section 10.3)

- [ ] **16.15** Replace `src/main.rs` `run()` with Phase 2 version (Section 12)
      Verify: `LLVM_SYS_181_PREFIX=$(llvm-config-18 --prefix) cargo build`

- [ ] **16.16** Write `examples/diode_op.ppr` (Section 0)

- [ ] **16.17** End-to-end run:
      ```bash
      export LLVM_SYS_181_PREFIX=$(llvm-config-18 --prefix)
      cargo run -- examples/diode_op.ppr
      ```
      Expected: `Id = 0.026752 A` (±1%)

- [ ] **16.18** Integration test `tests/e2e_openvaf_test.rs`:
      ```rust
      #[test]
      fn diode_op_simulation() {
          if std::env::var("LLVM_SYS_181_PREFIX").is_err() {
              eprintln!("LLVM_SYS_181_PREFIX not set — skipping OSDI test");
              return;
          }
          // parse + compile + run examples/diode_op.ppr
          // assert output contains "Id = " and non-zero value
      }
      ```

---

## 17. Error messages

| Situation | Error |
|-----------|-------|
| LLVM 18 not found at build time | `piperine-openvaf: LLVM 18 not found.` (build.rs) |
| VA source has syntax error | `error: openvaf: compilation failed with fatal diagnostic (see stderr)` |
| ngspice can't load `.osdi` | `error: pre_osdi: simulator error: command failed: pre_osdi /path/to/file.osdi` |
| OSDI model name mismatch | `error: runtime: simulator error: command failed: op` (unknown model during analysis) |

---

## 18. What Phase 2 does NOT do

- AC, noise, or transient OSDI analyses (OP only)
- Multiple `.ppr` files in one run
- Model-level vs instance-level parameter separation
- Reading OSDI metadata with `libloading` (VA parsed source suffices)
- Xyce OSDI support

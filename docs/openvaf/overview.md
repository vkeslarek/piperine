# OpenVAF / OSDI Overview

Piperine uses OpenVAF-Reloaded to compile Verilog-A device models to OSDI shared libraries, which ngspice loads at runtime.

## What is OSDI?

OSDI (Open Source Device Interface) is a standard ABI for compiled device models. An `.osdi` file is a shared library (`.so` on Linux) that ngspice can load via the `osdi` command. It provides pre-compiled, high-performance device models without requiring SPICE to interpret Verilog-A at runtime.

## What is OpenVAF?

OpenVAF-Reloaded is a Verilog-A compiler that produces OSDI libraries. Piperine uses it via the `piperine-openvaf` crate. The compiler:
- Reads `.ppr` or `.va` files containing `module` + `analog` blocks
- Performs symbolic differentiation to compute Jacobian entries
- Emits an `.osdi` shared library

## Workflow

```
my_device.ppr (or .va)
       │
       ▼
OpenVAF-Reloaded compiler (piperine-openvaf)
       │
       ▼
my_device.osdi (shared library)
       │
       ▼
ngspice loads: osdi my_device.osdi
       │
       ▼
Device available in SPICE netlists as .model + instance lines
```

## Automatic compilation

When you run Piperine on a `.ppr` file that contains analog modules, compilation and loading happen automatically:

1. `extract_va_modules()` — finds modules with `analog` blocks
2. `compile_va()` — compiles the file to `.osdi` (cached in `~/.cache/piperine/osdi/`)
3. `LibraryCompiler::pre_load()` — issues `osdi <path>` to ngspice before loading the netlist

The compiled `.osdi` is cached by source file path and mtime — recompilation only happens when the source changes.

## Cache location

```
~/.cache/piperine/osdi/
```

Override by setting a custom `cache_dir` in `src/main.rs`. The cache avoids re-compiling on every run.

## Supported Verilog-A subset

OpenVAF-Reloaded supports a substantial subset of Verilog-A. Key supported features:

- Analog blocks with branch contributions (`<+`)
- Parameters with ranges (`from [lo:hi]`)
- Standard math functions (`exp`, `log`, `sin`, `cos`, `sqrt`, `limexp`, etc.)
- Time derivatives (`ddt`)
- Temperature model variables (`$temperature`, `$vt`)
- `$abstime` for time-dependent models
- Multi-terminal devices
- `@` event (`@(initial_step)`, `@(final_step)`)

Limitations (not yet supported):
- `genvar` / generate blocks
- Verilog procedural blocks inside analog modules
- Hierarchical instances inside VA modules
- Some advanced noise models

See `docs/openvaf/writing_models.md` for details on what constructs work.

## ngspice integration

ngspice loads OSDI libraries with the `osdi` command (before `.circuit`). Piperine's worker handles this automatically:

```
piperine-worker receives: LoadOsdi { path }
→ issues to ngspice: "osdi /path/to/model.osdi"
→ confirms success
```

After loading, the model's `module_name` (from the VA `module` keyword) is used as the SPICE model type. Instances reference it via `.model` lines.

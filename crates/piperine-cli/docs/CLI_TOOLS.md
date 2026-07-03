# Piperine CLI & Decentralized Dependency Manager

This document serves as the final detailed specification for the Piperine Command Line Interface (CLI). The system is not just a wrapper around the compiler, but rather a complete infrastructure package wrapper, featuring manifest control (`Piperine.toml`) and a powerful **native git dependency resolution system**.

## 1. The Manifest (`Piperine.toml`)

`Piperine.toml` is the root of your project. The CLI maps the instructions written in this file to determine how and where to fetch third-party source code for your hardware or simulation libraries, as well as defining general metadata.

### General Structure
```toml
[project]
name = "my_spice_project"
version = "0.1.0"
authors = ["Author <author@email.com>"]
edition = "2024"

[dependencies]
# Remote dependency on a specific release branch
spice = { git = "https://github.com/vkeslarek/piperine-spice.git", version = "0.1.0" }

# Remote dependency explicitly pointing to the `develop` branch
std_analog = { git = "https://github.com/piperine/std_analog.git", branch = "develop" }

# Remote dependency pinned to a specific SHA commit hash
dsp = { git = "https://gitlab.com/corp/dsp.git", rev = "a1b2c3d4" }

# Remote dependency pointing to the latest tag referenced by origin
utils = { git = "https://github.com/piperine/utils.git" } 

# Local system path dependency
local_lib = { path = "../local_lib" }
```

### The Versioning Hash (`Piperine.lock`)
Whenever an instruction is resolved for the first time (or if it is modified), the CLI will iterate over the cloned git repository and extract its **HEAD Hash**, generating/updating the `Piperine.lock` file. This ensures absolute reproducibility. No updates will occur regardless of server-side modifications unless explicitly requested or if no lockfile exists.

---

## 2. The Dependency Command Tree

The CLI offers three literal commands designed to avoid manual editing of `Piperine.toml` and to cross-check the existence of packages before registering them in code:

### 2.1 `piperine add <name> [options]`
Safely adds libraries and packages to your project:
* When you add a package, the CLI will attempt to **download, clone, and checkout the specified tag immediately**.
* If the URL is incorrect, or the branch or commit does not exist, the CLI employs a "Fail-Fast" policy: **It will not save the package name to your `Piperine.toml` file**, leaving it intact.

**Supported arguments (versioning arguments are mutually exclusive):**
- `--git <url>`: The repository address
- `--version <x.y.z>`: The system will autocomplete by referencing the remotely formatted local release tag: `release/vx.y.z`
- `--branch <name>`: Points directly to a branch
- `--rev <hash>`: Checks out a Hash
- `--path <directory>`: Links a lib to a local file system directory

### 2.2 `piperine remove <name>`
The intelligent deletion routine:
1. Removes the dependency line from your TOML manifest.
2. Reconstructs the virtual dependency tree from the remaining libraries.
3. If it verifies that **no other sub-dependency internally uses this repository**, it deletes the cache directory under `target/deps/<name>`, cleaning up disk garbage.

### 2.3 `piperine tree`
Visualizes the resolved topology of all cloned libraries. Essential when dealing with huge AMS packages. It reflects exactly where the compiler's source code instances are being read from.
```text
$ piperine tree
my_project v0.1.0
├── spice (/home/user/my_project/target/deps/spice)
├── dsp (/home/user/my_project/target/deps/dsp)
```

---

## 4. Pipeline Commands & Integration

### 3.1 Resolving Transitions
Whenever you call **`piperine build`**, **`run`**, **`test`**, or **`check`**, the compiler enters a strict package validation routine before even processing the first AST:

1. `Resolver::new` from the `piperine-project` crate is invoked.
2. `Piperine.toml` and `Piperine.lock` are parsed.
3. All packages are placed into a validation hash.
4. **Strict Conflict Resolution**: If an external package A pulls `dsp v0.1.0` and an external package B pulls `dsp v0.2.0`, unlike other package managers, Piperine enforces a *HARD FAILURE*, throwing an unrecoverable error. The team must manually resolve the paths and calls to align versions; there is no tolerance for double versioning (since simulators deal with unified hardware).
5. `build_source_map` receives the complete library tree, iterating and injecting them as `SourceMap::add_namespace`.

This allows a developer to open their project's Phdl file and simply declare:
```phdl
use spice::sources::vsrc;

mod my_top {
    // ...
}
```
The CLI will instruct the compiler to fetch the package directly from `target/deps/spice/src/sources.phdl`.

### 3.2 Scaffolding (`piperine new <name>`)
Sets up the foundation of a new isolated repository, creating a directory and natively inserting a basic `Piperine.toml` alongside the `src/main.phdl` directory hierarchy.

### 3.3 The Internal Standard Library Fallback
If execution takes place within a repository where the CLI is still running from source (e.g., executing the CLI locally from within the Piperine clone directory on the tool developer's machine), it will dynamically inject `piperine::*` by tracking the parent folder `crates/piperine-lang/headers/` to enable development simulations, proving its toolchain flexibility.

## 4. Compilation & Execution Commands

The CLI acts as the main entry point to the Piperine compiler. All of the commands below inherently resolve dependencies via `Piperine.toml` before execution.

### 4.1 `piperine check [file]`
**Purpose:** Fast syntax and semantic validation.
- Parses and elaborates the abstract syntax tree and type-checks the code.
- If `[file]` is not provided, it automatically traverses the `src/` directory, discovering and validating all `.phdl` files.
- Ideal for fast feedback loops during coding or IDE/LSP integrations.

### 4.2 `piperine build [file]`
**Purpose:** Full project elaboration.
- Currently behaves similarly to `check` by fully elaborating the design.
- In the future, this command will act as the hook for exporting netlists, emitting SPICE decks, and generating Cranelift artifacts or `.osdi` extensions for OpenVAF models.
- Defaults to building `src/main.phdl` if no file is provided.

### 4.3 `piperine run [file] [--entry <module::fn>]`
**Purpose:** Direct execution of simulation benches.
- Evaluates the design and invokes the `BenchRunner`.
- If an `--entry` parameter is provided (e.g., `my_bench::dc_test`), it executes exclusively that simulation node.
- Otherwise, it evaluates and executes all `bench` entry points discovered within the specified file.
- Outputs the simulation execution trace, evaluating `$op()` and `$tran()` analog routines natively.

### 4.4 `piperine test [file] [--list]`
**Purpose:** Project-wide regression testing.
- Discovers and runs every simulation bench across the entire project (`src/**/*.phdl`).
- Validates successes and failures based on assertion triggers.
- The `--list` flag can be used to simply map and print all available test benches in the project without actually running them.

### 4.5 `piperine fmt [file]`
**Purpose:** Source code formatting.
- Automatically formats the Phdl files using the `TokenFormatter`.
- Corrects indentation, spacing, and structural layout of the source files.
- Operates in-place. Defaults to `src/main.phdl` if no target is specified.

### 4.6 `piperine clean`
**Purpose:** Cache and artifact cleanup.
- Deletes the `target/` directory for the active project.
- Useful for forcing the `Resolver` to re-fetch all cached git dependencies (`target/deps/`) or wiping old elaboration binaries.

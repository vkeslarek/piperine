# Piperine CLI (Refinement Document)

This document outlines the high-level design and functionality of the command-line interface (CLI) for Piperine. Inspired by Cargo (Rust's package manager), the CLI aims to provide an intuitive, standardized workflow for developing, compiling, and simulating Verilog-AMS projects.

## 1. Project Management (`Piperine.toml`)

To manage large analog/mixed-signal designs, Piperine introduces a project-based approach using a manifest file: `Piperine.toml` (similar to `Cargo.toml`).

### `Piperine.toml` Concept
*   **Project Metadata:** Defines the project name, version, authors, and default top-level module.
*   **Dependencies:** (Future) Allows fetching external Verilog-AMS libraries, OpenVAF models, or SPICE netlists from a centralized registry or git repositories.
*   **Workspace Organization:** Supports splitting a large chip design into smaller sub-packages (e.g., analog front-end, digital core, PLL).
*   **Profiles:** Defines compilation profiles (e.g., `dev` for fast compilation/debugging with `$display`, `release` for optimized ngspice simulations).

## 2. Core Commands (Cargo-Inspired)

### `piperine check`
*   **Purpose:** Fast syntax and semantic validation.
*   **Behavior:** Parses `.vams` files, checks for syntax errors, missing includes, and basic type resolution without fully elaborating or generating a netlist.
*   **Use case:** Ideal for fast feedback loops during coding or within IDE integrations/LSP.

### `piperine fmt`
*   **Purpose:** Code formatting.
*   **Behavior:** Automatically formats `.vams` and `.va` files according to standard Piperine style guidelines (indentation, whitespace, alignment of port declarations).
*   **Use case:** Keeping the codebase clean and enforcing a consistent style across the team.

### `piperine build`
*   **Purpose:** Elaboration and compilation.
*   **Behavior:** 
    *   Resolves all module instantiations and parameters.
    *   Generates the intermediate SPICE deck for the ngspice backend.
    *   Compiles Verilog-A device models into shared objects (`.osdi`) using OpenVAF if required.
*   **Use case:** Preparing the design for simulation or exporting it for other EDA tools.

### `piperine run`
*   **Purpose:** Execute the simulation.
*   **Behavior:** 
    *   Automatically runs `piperine build` if the design is outdated.
    *   Spawns the `piperine-worker` pool.
    *   Executes the continuous-time (ngspice) and discrete-time (procedural blocks) co-simulation.
    *   Streams `$display` outputs and waveform data to the terminal or a file.
*   **Use case:** Simulating a specific module or the default top-level design.

### `piperine test`
*   **Purpose:** Automated testing.
*   **Behavior:** 
    *   Discovers modules marked as testbenches or located in a `tests/` directory.
    *   Runs simulations in parallel.
    *   Reports successes and failures based on `$fatal`, `$error`, or assertion triggers.
*   **Use case:** Continuous Integration (CI) and regression testing.

## 3. Additional Suggested Commands

### `piperine init` / `piperine new`
*   **Purpose:** Scaffolding a new project.
*   **Behavior:** Creates a new directory structure (e.g., `src/`, `tests/`) and generates a skeleton `Piperine.toml`.

### `piperine clean`
*   **Purpose:** Removing build artifacts.
*   **Behavior:** Deletes the `target/` directory containing generated SPICE decks, compiled `.osdi` models, and temporary simulation data.

## 4. Next Steps for Development
*   Define the exact schema for `Piperine.toml` (sections, keys, data types).
*   Implement `piperine new` and `piperine check` as the first CLI milestones.
*   Integrate the CLI with the existing parser and `piperine-coordinator`.

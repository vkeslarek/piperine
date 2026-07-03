# Pre-Implementation Spec: Decentralized Dependency Management

This document serves as the implementation plan and technical specification for `piperine-cli`'s decentralized package management and scaffolding tools.

## 1. Features to Implement

### 1.1 Project Scaffolding
- **`piperine init`**: Initialize a `Piperine.toml` and basic project structure in the current directory.
- **`piperine add <git-url>`**: Automatically clone the package, extract its `Piperine.toml`, inject the entry into the local `Piperine.toml`, and download it into `target/deps/`.
- **`piperine remove <package>`**: Remove the package from `Piperine.toml` and delete it from `target/deps/`.
- **`piperine tree`**: Display a tree of the resolved dependencies (including transitive ones).

### 1.2 Git-Native Dependency Fetching
- Dependencies are defined in `Piperine.toml`:
  ```toml
  [dependencies]
  spice = { git = "https://github.com/piperine/spice.git", version = "0.0.1" }
  std_analog = { git = "...", branch = "main" }
  dsp = { git = "...", rev = "a1b2c3d4" }
  utils = { git = "..." } # Fallback to `latest` tag
  local_lib = { path = "../local_lib" }
  ```
- **Version Mapping**: `version = "0.0.1"` instructs the CLI to fetch/checkout the branch `release/v0.0.1`.
- **Storage**: All remote dependencies are cloned into `target/deps/<package_name>`.
- **Transitive Dependencies**: The CLI recursively reads `Piperine.toml` from fetched packages. All transitive dependencies are flattened into `target/deps/`.
- **Conflict Strategy**: If there are version conflicts (Package A wants `0.1.0` and Package B wants `0.2.0` of the same package), the resolution **fails immediately**. Manual user intervention is required.

### 1.3 `Piperine.lock`
- The system generates `Piperine.lock` recording the exact Git commit hash for every dependency.
- If the lockfile exists, it guarantees deterministic checks-outs, ignoring branch/tag rules unless `piperine update` is called.

### 1.4 Integration with the Compiler
- Before compilation/elaboration, the CLI injects all packages in `target/deps/` (and local paths) into `piperine_lang::SourceMap` as distinct namespaces.
- E.g., `spice` -> `target/deps/spice/src`.

---

## 2. Implementation Plan (How to Build It)

### Step 1: Parsing `Piperine.toml`
Currently, `piperine_project` might be doing basic parsing. We need to expand it:
- Create strongly typed structs via `serde` + `toml` to parse the `[dependencies]` table.
- **Types needed**:
  ```rust
  pub enum DependencySource {
      Git { url: String, req: GitRequirement },
      Path(PathBuf),
  }
  pub enum GitRequirement {
      Version(String), // e.g., "0.0.1" -> translates to "release/v0.0.1"
      Branch(String),
      Rev(String),
      Latest,          // Default if nothing specified
  }
  ```

### Step 2: Git Abstraction (`crates/piperine-project/src/git.rs`)
Create a module inside the `piperine-project` crate to handle Git operations natively using the **`git2`** (libgit2 bindings) crate:
- Use `git2::Repository::clone` to perform clones.
- Fetch updates via `git2::Remote::fetch`.
- Checkout specific revisions, branches, or tags using `set_head` and `checkout_head`.
- Extract the exact commit hash for the `Piperine.lock` generation.

### Step 3: Dependency Resolver (`crates/piperine-project/src/resolver.rs`)
This will be the engine that flattens the tree, living entirely inside `piperine-project`. The CLI will simply call it.
1. Start with the root `Piperine.toml` dependencies.
2. For each dependency, figure out its target path (`target/deps/<name>` or local `path`).
3. If it's a Git dependency, use `git2` to clone or fetch it. Checkout the requested target (or use the commit from `Piperine.lock` if it exists).
4. Parse the fetched package's `Piperine.toml`.
5. Recursively process its dependencies.
6. Track resolved versions in a `HashMap<String, GitRequirement>`. If a package is seen again with a *different* `GitRequirement`, return a `ConflictError` to fail-fast.
7. Return a flat map of `package_name -> local_path`.

### Step 4: Lockfile Generation (`Piperine.lock`)
- After resolution succeeds, iterate over the flat map.
- For Git dependencies, use `git2` to get the exact commit hash (`HEAD`).
- Serialize the flat map + hashes into `Piperine.lock`.

### Step 5: Wiring to CLI Commands
- **`piperine add` / `piperine remove`**: Modify `Piperine.toml` directly (using `toml_edit` to preserve comments/formatting) and then trigger the resolver to fetch.
  - **Sintaxe Expandida para `add`**:
    - `piperine add <nome> --git <url>`: Adiciona a dependência usando a tag `latest` por padrão.
    - `piperine add <nome> --git <url> --version <semver>`: Mapeia para a branch `release/v<semver>`.
    - `piperine add <nome> --git <url> --branch <branch>`: Usa uma branch específica.
    - `piperine add <nome> --git <url> --rev <commit>`: Fixa num commit específico.
    - `piperine add <nome> --path <caminho_local>`: Adiciona dependência local.
    - O comando deve primeiramente tentar resolver a dependência e baixar, falhando a adição no `Piperine.toml` caso o repositório ou versão não exista.
  - **Sintaxe Expandida para `remove`**:
    - `piperine remove <nome>`: Remove a entrada correspondente do `Piperine.toml` e, opcionalmente, limpa o cache em `target/deps/<nome>` (se nenhuma outra dependência transiente a utilizar).
- **`piperine tree`**: Trigger the resolver and print the hierarchy.
- **`piperine check`, `build`, `run`, `test`**:
  - Automatically call the resolver first.
  - Take the resulting flat map of `<name, path>`.
  - For each entry, add it to the `SourceMap`: `source_map.add_namespace(name, path.join("src"))`.

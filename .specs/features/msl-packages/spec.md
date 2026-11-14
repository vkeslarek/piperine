# MSL Packages Specification

**Approved**: 2026-11-14 - user: "Dispara o fable para especificar o project e despachar"

## Problem Statement

A model cannot use code from another package. `docs/spec/` §3.4 says only "a package of models is a crate"; `Design::load(root)` reads one tree of files plus prelude sources a host injects by hand; `piperine-project` still carries the retired Verilog-AMS pipeline (`SourceMap`, plugin sections, release-binary fetch) and does not build. The standard library (`msl-stdlib`, next) needs a crate to live in and a way to be found.

This feature gives MSL Rust's package model, whole: a crate reaches another only through a dependency named in its manifest (U1), `std` is a crate embedded in the binary (U2), and `piperine-project` is cut down to manifest, dependencies and lockfile (U3).

## Goals

- [ ] `docs/spec/` defines crates (§3.5) and packages (§3.6) normatively: crate root, dependency names in paths, `crate`/`self`/`super`, `pub` across crates, the implicit `std`, the manifest, path and git dependencies, the lockfile, the ambiguity code `E0163`, and seven error codes of a new phase `package`.
- [ ] `piperine_lang::pom::Design` is built over a crate graph given as plain data, holds every crate as authored, resolves names across crates by Rust's rules and offers a `Crate` handle. `piperine-lang` gains no dependency.
- [ ] `std` is a crate embedded in `piperine-lang`; the prelude is `std::prelude`. The builder's prelude sources are gone.
- [ ] `piperine-project` reads `Piperine.toml`, resolves path and git dependencies into that graph, keeps `Piperine.lock`, and loads a project with one call. Everything else in the crate is deleted. It builds with zero warnings.
- [ ] `cargo test -p piperine-lang` and `cargo test -p piperine-project` are green at every commit from the task that touches each.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| The content of the standard library (`std/*.msl` as the normative source of Part VII, the galleries building clean, D4) | next feature `msl-stdlib` (U4). This feature ships the mechanism and a placeholder: a root file and an empty `prelude` |
| Include or search paths (`-I`), an environment variable, any way to find a crate that is not a named dependency | U1: "-I é ruim demais, faremos modelo RUST 100%" |
| A package registry and version selection among several published versions | there is no registry; `path` and `git` locate one package each. `version` on a dependency is a check, not a selection |
| Release-binary fetch, `[plugins]` and `[plugin]` sections, plugin trust entries in the lockfile, the VAMS `SourceMap` recipe, `GitSource` (the `owner/repo` shorthand of `piperine add`) | U3: deleted, "para podermos desenvolver depois" |
| The CLI, the language server, the Python binding, `piperine-plugin`, `piperine-api`, the root crate | they do not build against the MSL front end today and stay as they are (brief). Their calls into the deleted surface are not repaired here |
| Workspaces, features, optional and dev dependencies, build scripts, `[lib] path`, several crates per package, a package inside a subdirectory of a git repository (`subdir`) | not needed to name a dependency; each is a later, separate decision |
| `--locked` / `--offline` switches | they are CLI surface; the resolver already works without the network when the lockfile and the checkouts are present (PKG-13 AC 8) |
| Type checking across crates (trait coherence, the orphan rule), elaboration, monomorphization | later features; the POM resolves and classifies names only |

---

## Assumptions & Open Questions

U1 to U6 are the user's decisions (`context.md`, 2026-11-14). The other rows are the planner's choices inside them; each is open to correction and is listed in the planner's hand-off.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| U1 Search paths | none: a crate is reached only through a dependency named in the manifest | user decision | y - user: "-I é ruim demais, faremos modelo RUST 100%." |
| U2 Where `std` lives | embedded in `piperine-lang` (`include_str!` of `crates/piperine-lang/std/*.msl`) | user decision | y - user: "std fica embutida o binário." |
| U3 Scope of `piperine-project` | manifest, path and git dependencies, lockfile; the rest is deleted | user decision | y - user: "trazer ele apenas para dependencias, manifesto e lockfile mesmo" |
| U4 Order | this feature first; `msl-stdlib` after | user decision | y - user: "Legal, gostei!" |
| U5 Roles | Sonnet implements, Opus verifies | user decision | y - standing rule |
| Where the rules live in `docs/spec/` | two new sections of Part I: §3.5 Crates (language) and §3.6 Packages (manifest, dependencies, lockfile). No new part, no new file | §3.4 is already "Files and imports"; Part I owns `E01xx`, so the codes need no new prefix; `pom_docs` maps a code to a numbered section | y - planner default |
| Crate root and extension | `src/lib.msl`, fixed; sources are `.msl` | Cargo's layout. A package holds one library crate: MSL has no `main`, the host picks the top module (G.17). Closes the open half of MD-55 | y - planner default |
| U6 Name clash between a local name and a dependency | Rust 2018 exactly: in an ordinary path the item or import wins, silently; in a `use` path the clash is an ambiguity error, never hidden. The help names the two ways out: rename the dependency in the manifest (`package = "…"`) or write `crate::<name>` for the local one | user decision (`context.md` U6) | y - user: U6 |
| Code of the ambiguity | `E0163`, in §3.5, phase `elaboration` | the next free code after `E0162`; a new condition gets its own code (§0.2) | y - planner default inside U6 |
| Unknown crate, private item across crates (no new code) | the existing `E0161`, with a help line for the crate case | §0.2: one condition, one code. "A path that does not resolve" already is that condition | y - planner default |
| A crate's own name | not in scope inside the crate; `crate` names its root | Rust's rule | y - planner default |
| The prelude | the `pub` items of `std::prelude`, implied in every file of every crate, `std`'s own files included; below dependency names, above core names | closest to Rust while keeping §3.4.2 ("imported implicitly into every file") and §22 rule 1 true. No exception for `std` keeps one rule | y - planner default |
| Phase of manifest errors | a fifth phase, `package` | a manifest is read before any source is parsed; calling it `elaboration` would stretch §0.3's definition, and `parse` rows are bound to parser fixtures | y - planner default |
| Codes | `E0170` to `E0176` in §3.6 | free in Part I; `E016x` stays with §3.4 | y - planner default |
| Manifest tables | `[package]` (`name`, `version` required; `authors`, `description` optional) and `[dependencies]`; anything else is an error | the smallest set that names a package. `[project]` and `edition` of the old manifest are dropped: Cargo says `[package]`, and the language has one edition | y - planner default |
| Names | an identifier of A.1 that is no keyword and not `std`; no hyphens | the name is written in paths. Cargo's hyphen-to-underscore mapping is a second spelling of one name | y - planner default |
| Git references | `branch`, `tag`, `rev`, at most one; none is the default branch | Cargo's keys. The old code read `version` as the tag `release/v<version>` and "latest" as a ref named `latest`: both are conventions of the deleted plugin flow | y - planner default |
| `version` on a dependency | a check: Cargo's caret form `M`, `M.m` or `M.m.p` against the located package's `version`; operators are not accepted | cheap (no new crate), catches a path or branch that drifted to an incompatible release | y - planner default; brief lists `version` |
| Renaming a dependency | `package = "real"`, as in Cargo | cheap, and the only way to depend on two packages of one name or to escape a name clash | y - planner default; brief: "only if cheap" |
| Lockfile | `Piperine.lock` of the root package only; git packages only; TOML with `version = 1` | a path package is pinned by its location; Cargo ignores dependencies' lockfiles too | y - planner default |
| Git checkouts | `<root package>/target/deps/<sanitized url>/<commit>` | the existing code already uses `target/deps`; one directory per commit lets two revisions of one repository coexist; no hash crate needed | y - planner default |
| Front-end input | `CrateGraph`: crates (name, root path) and named edges, acyclic by construction, the first crate is the root | the brief: `piperine-lang` takes a resolved graph as plain data and must not read TOML or git | y - settled by the brief |
| What replaces `DesignBuilder::prelude` | `std_source(path, text)`: virtual files of the `std` crate, replacing the embedded ones | tests and docs need a small library of their own; a "prelude source" is not a thing in the package model | y - planner default |
| File numbering | the root crate's files first, then the other crates in graph order, `std` last | `FileId(0)` stays the root file, which most existing tests and every single-file caller assume | y - planner default |
| Listings of `Design` | typed collections list the root crate; `Crate` lists any crate; by-name lookups prefer the root crate, then crate order | carries `msl-polish` A-12 (project first) over from "non-prelude files" to crates | y - planner default |
| Diagnostics of `piperine-project` | `piperine_lang::msl::Diagnostic` with the manifest text, rendered by the same `miette` report | one diagnostic type and one renderer in the workspace | y - planner default |
| Pace | one task per commit, tests first; Sonnet workers, an Opus Verifier | the brief | y - settled by the brief |

**Open questions:** none - U1 to U6 are answered; every planner default above is stated with its reason and is listed in the hand-off for the user to correct.

**Implicit-requirement dimensions.** Input validation & bounds: PKG-03 (graph names, cycles, foreign ids), PKG-10 (every manifest key, name, version and requirement). Failure / partial failure: PKG-04 AC 5 (an unreadable crate root does not stop the build), PKG-10 AC 9 (every fault of a manifest is reported), PKG-14 AC 3 and 4 (a package error returns no design; a model error returns one). Idempotency / retry: PKG-13 AC 4 (two resolutions, identical lockfile), PKG-13 AC 2 (a re-run keeps the pin). Auth boundaries & rate limits: N/A because the resolver passes the URL to git and stores no credential; no server is contacted on a locked build. Concurrency / ordering: N/A because resolution is one sequential walk and the design is immutable; two processes resolving one project at once are not supported and not specified. Data lifecycle: PKG-13 AC 6 (a stale lockfile entry is dropped); checkouts under `target/deps` are a cache the user may delete. Observability: every failure is a coded diagnostic with a span (PKG-10 AC 3); no logging is added. External-dependency failure: PKG-12 AC 3 (repository or reference unreachable is `E0172`), PKG-13 AC 8 (no network needed once locked). State-transition integrity: PKG-03 AC 2 to 4 (a rejected insertion leaves the graph unchanged), PKG-13 AC 7 (an invalid lockfile fetches nothing).

---

## User Stories

### P1: The specification says what a crate and a package are ⭐ MVP

**User Story**: As a model author and as the author of every later tool, I want the specification to say what a crate is, how one crate names another and what a manifest may hold so that packages behave the same in every implementation.

**Why P1**: today §3.4 says only "a package of models is a crate". Nothing else in this feature can be tested against a rule until the rule is written (MD-57: the specification leads).

**Acceptance Criteria** (PKG-01, The specification defines crates (§3.5)):

1. The specification SHALL hold in Part I a section `3.5 Crates` that defines the terms crate, crate root and dependency name, each bold at one defining occurrence, and SHALL reword the lead of §3.4 to point at it.
2. The specification SHALL state in §3.5 the order in which the first segment of a path is looked up: the enclosing scopes, the items and imports of the enclosing namespace, the dependency names of the crate, the prelude, the core names.
3. The specification SHALL state in §3.5 that, where an item or an import of the enclosing namespace has the name of a dependency, an ordinary path resolves to the item or import without a diagnostic and a `use` path is ambiguous (`E0163`), and that no leading `::` exists.
4. The specification SHALL state in §3.5 that `crate` names the root of the crate that contains the path, that a crate's own name is not in scope inside it, and that `super` at a crate root is `E0161`.
5. The specification SHALL state in §3.5 that an item is visible from another crate only when it and every namespace on the path from its crate root are `pub`.
6. The specification SHALL state in §3.5 that `std` is a dependency of every crate that no manifest names and no dependency may be called, and SHALL define the prelude in §3.4.2 as the `pub` items of `std::prelude`.
7. The `E0161` row of §3.4 SHALL name a first segment that is not a dependency of the crate.
8. The specification SHALL list `extern crate` items in the removed-forms table of Appendix B.3.6 with the code `E0900` and the manifest's `[dependencies]` as what to write instead.
9. IF a source holds `extern crate util;` THEN the parser SHALL report exactly one `E0900` whose message says that `extern crate` is not part of the language and whose help names `[dependencies]`.
10. The error table of §3.5 SHALL define `E0163`, of phase `elaboration`, as a `use` path whose first segment names both a dependency and an item or import of the enclosing namespace, and the section SHALL name the two ways out: `package = "…"` under another key in the manifest, and `crate::<name>` for the local one.

**Acceptance Criteria** (PKG-02, The specification defines packages (§3.6)):

1. The specification SHALL list `package` in §0.3 as a fifth phase, raised while manifests are read and the crate graph is fixed, and SHALL require in §0.3 rule 3 that an implementation reject a model that meets a condition of that phase.
2. The specification SHALL fix in a section `3.6 Packages` the layout of a package: the manifest `Piperine.toml` in the package directory, the crate root `src/lib.msl`, the source extension `.msl`, and `mod name;` loading `name.msl` or `name/mod.msl`.
3. The specification SHALL hold in §3.6 one table of manifest keys with a row for each of `name`, `version`, `authors`, `description` under `[package]` and `path`, `git`, `branch`, `tag`, `rev`, `version`, `package` under a dependency, and SHALL state that any other table or key is `E0171`.
4. The specification SHALL state in §3.6 that a package name and a dependency name are identifiers of Appendix A.1 that are no keyword and not `std`, and that a version is `MAJOR.MINOR.PATCH`.
5. The specification SHALL state in §3.6 that a dependency has exactly one of `path` and `git`, that a `git` dependency takes at most one of `branch`, `tag` and `rev`, and that none of them means the repository's default branch.
6. The specification SHALL give in §3.6 the table of version requirements with the eight forms `1.2.3`, `1.2`, `1`, `0.2.3`, `0.2`, `0.0.3`, `0.0`, `0` and the range each admits.
7. The specification SHALL state in §3.6 that a crate is reached through a named dependency only (no search path, no registry, no environment variable) and that the dependencies form no cycle.
8. The specification SHALL state in §3.6 the lockfile rule: `Piperine.lock` beside the root manifest records the commit of each git package under the keys `name`, `version`, `source` and `commit` with a top-level `version = 1`, and the recorded commit is used while the dependency's source is unchanged.
9. The error table of §3.6 SHALL define `E0170`, `E0171`, `E0172`, `E0173`, `E0174`, `E0175` and `E0176`, each of phase `package`.
10. Appendix G SHALL list source search paths and a package registry in G.0 and SHALL record the decisions of this feature as closed rows `P1` to `P6`, each citing §3.5 or §3.6.
11. The glossary index of §0.9 SHALL hold rows for crate, crate root, dependency, package, manifest and lockfile, and the file map of §0.8 SHALL name crates and packages in the row of Part I.

**Independent Test**: read §3.5 and §3.6 and answer: where is the crate root, what does `util::X` mean, which keys may a manifest hold, which code does a cycle get.

---

### P1: A design is built over a crate graph ⭐ MVP

**User Story**: As a host (the type checker, the elaborator, the LSP), I want one `Design` over several crates, each read back exactly as written, so that a model can use a module of a dependency and I can still walk every crate's own tree.

**Why P1**: it is the mechanism of the feature: without it a second crate cannot be named at all.

**Acceptance Criteria** (PKG-03, The crate graph is plain data):

1. WHEN crates and dependencies are added to a `CrateGraph` THEN the graph SHALL return ids in insertion order and give back each crate's name, root path and dependencies as they were added.
2. IF a crate name or a dependency name is not an identifier, is a keyword, or is `std` THEN the graph SHALL return `GraphError::InvalidName` and stay unchanged.
3. IF a dependency would close a cycle, a crate that depends on itself included, THEN the graph SHALL return `GraphError::Cycle` with the crate names of the cycle in order and stay unchanged.
4. IF a crate already has a dependency of the given name THEN the graph SHALL return `GraphError::DuplicateDependency`.
5. IF a `CrateId` does not belong to the graph THEN the graph SHALL return `GraphError::UnknownCrate` and SHALL NOT panic.
6. The `piperine-lang` crate SHALL depend on no manifest or git library: its `Cargo.toml` names neither `toml` nor `git2` nor `piperine-project`.

**Acceptance Criteria** (PKG-04, A design holds its crates as authored):

1. WHEN a design is built from one root file or from virtual sources without a graph THEN it SHALL hold a root crate named after the stem of the root file, and `FileId(0)` SHALL be that crate's root file.
2. The design SHALL hold in every build a crate named `std` after every other crate.
3. WHEN a design is built from a graph THEN it SHALL hold one crate per crate of the graph, in graph order with the first as root crate, each with the name and the dependencies of the graph.
4. WHEN a file of a crate other than the root holds `mod b;` THEN the design SHALL load `b.msl` or `b/mod.msl` beside that crate's root file.
5. IF the root file of a crate cannot be read THEN the design SHALL report one `E0161` that names the path, on that crate's empty root file, and SHALL still build the other crates.
6. The design SHALL keep the declarations of each crate under that crate's own root namespace, and every file of every crate SHALL convert back (`to_ast`) to the tree the parser returned.
7. WHEN a graph is built from disk with `Design::load_crates` THEN the design SHALL equal, by `dump()` and by crate list, the one built from the same files given as virtual sources.

**Acceptance Criteria** (PKG-05, Names resolve across crates by Rust's rules):

1. WHEN a path starts with a dependency name THEN the POM SHALL resolve its first segment to the root namespace of that crate and its last segment to the item named.
2. WHEN a file imports a module of a dependency (`use util::cells::Cell;`) and places it THEN `Instance::module()` SHALL be the module declared in the dependency and the design SHALL hold no diagnostic.
3. WHEN a path in a file of a dependency starts with `crate` THEN the POM SHALL resolve it to the root of that dependency, not to the root crate.
4. WHEN a dependency is declared under a name other than the crate's own THEN the POM SHALL resolve the dependency name and SHALL report `E0161` for the crate's own name.
5. WHEN a namespace item or an import has the name of a dependency THEN the POM SHALL resolve an ordinary path (in an expression, a type or a pattern) that starts with that name to the item or import and SHALL report no diagnostic.
6. IF the first segment of a `use` path names both a dependency and an item or import of the enclosing namespace THEN the POM SHALL report exactly one `E0163` at that path, whose help names `package = "…"` in the manifest and `crate::<name>`, SHALL leave the path's segments unresolved, and SHALL report no `E0161` for it.
7. WHEN that `use` path is written `crate::<name>::..` THEN the POM SHALL resolve it to the local item and report no diagnostic.
8. WHEN a path starts with `std` in a crate whose graph entry declares no dependency THEN the POM SHALL resolve it to the root of the `std` crate.
9. WHEN a file of a dependency crate uses a `pub` item of `std::prelude` by its single name THEN the POM SHALL resolve it without an import.
10. WHEN a dependency name and a `pub` item of `std::prelude` are the same name THEN the POM SHALL resolve the name to the dependency.
11. IF a path starts with the name of a crate that is not a dependency of the containing crate (a dependency of a dependency, or the crate's own name) THEN the POM SHALL report one `E0161` whose help names `[dependencies]` of `Piperine.toml`.
12. IF a path names an item of another crate that is not `pub` THEN the POM SHALL report one `E0161` whose message says the item is private.
13. IF a path names a `pub` item inside a namespace of another crate that is not `pub` THEN the POM SHALL report one `E0161` at that path.
14. IF a path at the root of a dependency crate starts with `super` THEN the POM SHALL report one `E0161`.
15. WHEN a file glob-imports a dependency (`use util::*;`) THEN the POM SHALL bring its `pub` items only: a `pub` name resolves and a private one is `E0161`.
16. WHEN a dependency re-exports an item of its own dependency with `pub use` THEN the POM SHALL resolve the item through the re-exporting crate from a crate that does not depend on the item's crate.

**Independent Test**: build a graph `app -> util` from virtual sources, place `util::cells::Cell` in `app`, and read `instance.module().krate().name() == "util"`.

---

### P1: `std` is a crate inside the binary ⭐ MVP

**User Story**: As a model author, I want the standard library to be there without a manifest entry or a search path so that `Bit` and `std::prelude::Bit` mean the same thing in every project.

**Why P1**: user decision U2. It also replaces the builder-injected prelude sources, which no package can express.

**Acceptance Criteria** (PKG-06, `std` is embedded and its prelude is implicit):

1. The standard library embedded in `piperine-lang` SHALL build with zero diagnostics, and its root SHALL declare a `pub` namespace `prelude`.
2. WHEN a file uses by its single name a `pub` item of `std::prelude` THEN the POM SHALL resolve the name to that item without an import.
3. IF a file uses by its single name an item of `std::prelude` that is not `pub`, or a `pub` item of another namespace of `std`, THEN the POM SHALL report `E0161`.
4. WHEN the builder is given `std_source` files THEN the design SHALL build the standard library from them in place of the embedded one, the first being its root.
5. WHEN a namespace item and a `pub` item of `std::prelude` have one name THEN the POM SHALL resolve the name to the namespace item.
6. The design SHALL number the files of the standard library after every other file.
7. WHEN a file of the standard library uses by its single name a `pub` item of `std::prelude` THEN the POM SHALL resolve it as in any other file.

**Independent Test**: build a design from one empty source: zero diagnostics, and the last crate is `std`.

---

### P1: A package loads from its manifest ⭐ MVP

**User Story**: As a project author, I want `Piperine.toml` to name my package and its dependencies and one call to load the whole design so that I never pass a path on a command line.

**Why P1**: user decisions U1 and U3: the manifest is the only way to reach another crate, and `piperine-project` is cut down to it.

**Acceptance Criteria** (PKG-10, A manifest is read and validated):

1. IF a package directory holds no `Piperine.toml` THEN the manifest reader SHALL return one `E0170` that names the missing path.
2. IF a manifest is not valid TOML or not UTF-8 THEN the manifest reader SHALL return one `E0170` whose span is the offending text.
3. WHEN a package error is rendered THEN the output SHALL be a `miette` graphical report with the code, `Piperine.toml:line:col` of the span start, the source line and an underline as long as the span.
4. WHEN a manifest is valid THEN the manifest reader SHALL give the package's `name`, `version`, `authors` and `description` as written.
5. IF a manifest holds a table or a key that §3.6 does not list THEN the manifest reader SHALL report one `E0171` at that key whose message names it.
6. IF `[package]`, its `name` or its `version` is missing, or a key has a value of the wrong type, THEN the manifest reader SHALL report one `E0171` naming the key.
7. IF a package name is not an identifier, is a keyword, or is `std` THEN the manifest reader SHALL report one `E0171` at the value.
8. IF a version is not three decimal integers separated by dots THEN the manifest reader SHALL report one `E0171` at the value.
9. WHEN a manifest has several faults THEN the manifest reader SHALL report one diagnostic per fault, in source order.
10. WHEN a manifest declares dependencies THEN the manifest reader SHALL give each one's name, source (`path` or `git`), git reference, version requirement and `package` name, in source order.
11. IF a dependency has neither `path` nor `git`, or both, THEN the manifest reader SHALL report one `E0171` at the dependency's key.
12. IF a dependency has more than one of `branch`, `tag` and `rev`, or has one of them beside `path`, THEN the manifest reader SHALL report one `E0171`.
13. IF a dependency name is not an identifier, is a keyword or is `std`, or its `version` is not `M`, `M.m` or `M.m.p`, THEN the manifest reader SHALL report one `E0171` at the name or at the value.

**Acceptance Criteria** (PKG-11, Path dependencies become a crate graph):

1. WHEN a manifest at any depth of the graph names a `path` dependency THEN the resolver SHALL locate it relative to the directory of the manifest that names it.
2. WHEN a package graph is resolved THEN the resolver SHALL return a `CrateGraph` whose first crate is the root package, each crate named by its package name with the root `<package directory>/src/lib.msl`, and each edge named by its dependency name.
3. WHEN two dependencies locate the same package directory THEN the resolver SHALL make one crate of it with one edge from each dependent.
4. IF the directory of a `path` dependency holds no manifest THEN the resolver SHALL report one `E0172` at the dependency's key in the manifest that names it.
5. IF a package holds no `src/lib.msl` THEN the resolver SHALL report one `E0175` that names the missing path.
6. IF the package located for a dependency has a name other than the dependency's (or than its `package` key) THEN the resolver SHALL report one `E0173` that names both.
7. WHEN a dependency carries `package = "real"` THEN the resolver SHALL locate the package named `real` and name the edge by the dependency's key.
8. WHEN a dependency carries a `version` requirement THEN the resolver SHALL accept exactly the versions the table of §3.6 admits for it.
9. IF the located package's version does not satisfy the dependency's `version` THEN the resolver SHALL report one `E0173` that names the requirement and the version found.
10. IF the dependencies form a cycle THEN the resolver SHALL report one `E0174` at the dependency key that closes it, whose message lists the chain of package names.
11. IF a package names itself as a dependency THEN the resolver SHALL report one `E0174`.

**Acceptance Criteria** (PKG-14, One call loads a project):

1. WHEN `find_project_root` is given a directory THEN it SHALL return the nearest ancestor (itself included) that holds `Piperine.toml`, and none when there is none.
2. WHEN `load_project` is given a package with a path dependency THEN it SHALL return a design whose root crate has the package's name, that holds one crate per package plus `std`, that links an instance into the dependency, and that has zero diagnostics.
3. IF a package error occurs THEN `load_project` SHALL return `Err` with the package errors and no design.
4. WHEN a source file of a package has a language error THEN `load_project` SHALL return `Ok` with a design that carries the diagnostic.

**Acceptance Criteria** (PKG-15, `piperine-project` is manifest, dependencies and lockfile only):

1. The `piperine-project` crate SHALL hold no release-fetch, plugin or `SourceMap` surface: no `ureq`, `sha2`, `serde_json` or `thiserror` in its `Cargo.toml`, no `release.rs`, `source_map.rs` or `tests/release.rs`, and no occurrence of `plugin` in its sources.
2. The `piperine-project` crate SHALL report exactly the codes of the specification's `package`-phase rows.
3. The `piperine-project` crate SHALL build, tests included, with no warning.

**Independent Test**: in a temporary directory write `app` and `util` with their manifests, call `load_project(app)`, and read a clean design with three crates.

---

### P2: Crates are reflected, serialized and documented

**User Story**: As a tool author, I want to list crates, ask any declaration for its crate, serialize a multi-crate design and read all of it in `docs/pom/` so that packages are first-class in the object model.

**Why P2**: the object model stays usable without it (S2 resolves names), but reflection and the reference docs are what the next consumers read.

**Acceptance Criteria** (PKG-07, Reflection reaches crates):

1. WHEN a crate is read through its handle THEN `Crate` SHALL give its name, its root namespace, its files in load order, and whether it is the root crate or `std`.
2. WHEN a declaration or a source file is asked for its crate THEN `Reflect::krate` and `SourceFile::krate` SHALL return the crate that holds it.
3. WHEN a crate is asked for its dependencies THEN `Crate::dependencies()` SHALL list the declared ones as (dependency name, crate) in declaration order without `std`, and `Crate::dependency(name)` SHALL return the crate under that name.
4. The typed collections of `Design` (`modules`, `functions`, `structs`, `enums`, `traits`, `consts`, `impls`, `namespaces`, `natures`) SHALL list the root crate only, and `Crate::decls()` and `Crate::modules()` SHALL list the declarations of their crate only.
5. WHEN a by-name lookup of `Design` (`module`, `function`, `struct_named`, `nature`) has matches in several crates THEN it SHALL return the root crate's, else the first in crate order.
6. WHEN crates are looked up THEN `Design::crates()` SHALL list them in crate order, `Design::krate(name)` SHALL return the first of that name, and `root_crate()` and `std_crate()` SHALL return those two.
7. WHEN `Crate::find` is given the text `qualified_name()` prints for a named declaration of that crate THEN it SHALL return that declaration.

**Acceptance Criteria** (PKG-08, A multi-crate design serializes and is validated):

1. WHEN a design of several crates is written to JSON and read by `Design::from_json` THEN the result SHALL equal the original, crates and dependencies included.
2. IF the JSON of a design holds a crate link out of range (a file's crate, a dependency's crate, the root crate, the `std` crate, a crate's root file or root declaration) THEN `Design::from_json` SHALL return `Err` naming the link and SHALL NOT panic.
3. IF the crates of a design's JSON form a dependency cycle THEN `Design::from_json` SHALL return `Err`.
4. IF a crate's root declaration is not the root namespace of its own root file, or that file is tagged with another crate, THEN `Design::from_json` SHALL return `Err`.

**Acceptance Criteria** (PKG-09, The reference docs describe crates):

1. The pages of `docs/pom/` SHALL give the scope order with the dependency names and the `std` prelude, and SHALL NOT mention prelude sources or `.prelude(`.
2. The page `docs/pom/mapping.md` SHALL hold a row that maps a crate to `CrateData`, `Crate` and the lookups that reach it.
3. The `rust` examples of `docs/pom/*.md` SHALL run as doctests of `pom::guide` with none failing.

**Independent Test**: `design.crates()` prints `app`, `util`, `std`; the JSON of the design reads back equal.

---

### P2: Git dependencies are fetched and pinned

**User Story**: As a project author, I want a dependency on a git repository, pinned by a lockfile, so that a build is the same tomorrow and offline.

**Why P2**: path dependencies already make packages usable; git and the lockfile make them shareable. The existing `git.rs` is reused.

**Acceptance Criteria** (PKG-12, Git dependencies):

1. WHEN a `git` dependency names no reference THEN the resolver SHALL check out the head of the repository's default branch under `<root package>/target/deps` and root the crate at `src/lib.msl` of that checkout.
2. WHEN a `git` dependency names a `branch`, a `tag` or a `rev` THEN the resolver SHALL check out the commit it names.
3. IF a repository cannot be fetched, or holds no such branch, tag or commit, THEN the resolver SHALL report one `E0172` at the dependency's key that carries git's message.
4. WHEN two dependencies name one repository at two commits THEN the resolver SHALL make two crates with two checkouts.
5. WHEN two dependencies name one repository at one commit THEN the resolver SHALL make one crate.
6. WHEN a package fetched by git names a `path` dependency THEN the resolver SHALL locate it relative to the manifest inside the checkout.

**Acceptance Criteria** (PKG-13, The lockfile pins git packages):

1. WHEN a resolution locates a git package THEN the resolver SHALL write `Piperine.lock` beside the root manifest with `version = 1` and one `[[package]]` entry holding `name`, `version`, `source` and a 40-digit `commit`.
2. WHILE a lockfile entry's `source` equals a dependency's source THEN the resolver SHALL check out the recorded commit, also after the branch has moved.
3. WHEN a graph has no git package and no lockfile exists THEN the resolver SHALL create none.
4. WHEN an unchanged project is resolved twice THEN the resolver SHALL leave a byte-identical lockfile, its entries sorted by `name` then `source`.
5. WHEN a resolution is run with the update option THEN the resolver SHALL resolve every branch again and rewrite the lockfile with the new commits.
6. WHEN a git dependency is removed, or its source changes, THEN the resolver SHALL drop or replace its lockfile entry.
7. IF the lockfile is not valid TOML or its `version` is not `1` THEN the resolver SHALL report one `E0176` and SHALL fetch nothing.
8. WHILE the checkout of a locked commit exists THEN the resolver SHALL NOT contact the repository.
9. The resolver SHALL read the lockfile of the root package only: a `Piperine.lock` inside a dependency does not pin anything.

**Independent Test**: depend on a local repository by branch, resolve, commit to the branch, resolve again: the lockfile still holds the first commit.

---

## Edge Cases

Each is covered by the criterion named.

- IF a dependency key and a local `mod` have one name THEN the POM SHALL resolve an ordinary path to the `mod` and report nothing (PKG-05 AC 5).
- IF a `use` path starts with that name THEN the POM SHALL report `E0163` and no `E0161` (PKG-05 AC 6).
- IF a manifest names itself through `path = "."` THEN the resolver SHALL report `E0174`, not recurse (PKG-11 AC 11).
- WHEN two dependents reach one package directory through two spellings of its path THEN the resolver SHALL make one crate (PKG-11 AC 3).
- IF a crate's root file is missing THEN the design SHALL still hold the crate, empty, with one `E0161` (PKG-04 AC 5).
- WHEN the embedded standard library is replaced by `std_source` THEN the design SHALL hold no embedded file (PKG-06 AC 4).
- IF a lockfile names a commit for a dependency whose `branch` was edited THEN the resolver SHALL resolve the branch again (PKG-13 AC 6).
- IF the JSON of a design is edited so that two crates depend on each other THEN `Design::from_json` SHALL return `Err` (PKG-08 AC 3).
- WHEN a version requirement is `0.0.3` THEN the resolver SHALL accept `0.0.3` only (PKG-11 AC 8).
- IF a manifest still has the retired `[project]` table THEN the manifest reader SHALL report `E0171` at it (PKG-10 AC 5).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| PKG-01 | P1: The specification says what a crate and a package are | Tasks | Pending |
| PKG-02 | P1: The specification says what a crate and a package are | Tasks | Pending |
| PKG-03 | P1: A design is built over a crate graph | Tasks | Pending |
| PKG-04 | P1: A design is built over a crate graph | Tasks | Pending |
| PKG-05 | P1: A design is built over a crate graph | Tasks | Pending |
| PKG-06 | P1: `std` is a crate inside the binary | Tasks | Pending |
| PKG-07 | P2: Crates are reflected, serialized and documented | Tasks | Pending |
| PKG-08 | P2: Crates are reflected, serialized and documented | Tasks | Pending |
| PKG-09 | P2: Crates are reflected, serialized and documented | Tasks | Pending |
| PKG-10 | P1: A package loads from its manifest | Tasks | Pending |
| PKG-11 | P1: A package loads from its manifest | Tasks | Pending |
| PKG-12 | P2: Git dependencies are fetched and pinned | Tasks | Pending |
| PKG-13 | P2: Git dependencies are fetched and pinned | Tasks | Pending |
| PKG-14 | P1: A package loads from its manifest | Tasks | Pending |
| PKG-15 | P1: A package loads from its manifest | Tasks | Pending |

**Coverage:** 15 requirements, 117 acceptance criteria, 15 mapped to tasks, 0 unmapped.

### Requirements, their criteria and their tasks

| Requirement | What | Criteria | Tasks |
| ----------- | ---- | -------- | ----- |
| PKG-01 | The specification defines crates (§3.5) | 10 | T1, T4 |
| PKG-02 | The specification defines packages (§3.6) | 11 | T2, T3, T5 |
| PKG-03 | The crate graph is plain data | 6 | T6 |
| PKG-04 | A design holds its crates as authored | 7 | T8, T9 |
| PKG-05 | Names resolve across crates by Rust's rules | 16 | T10, T11 |
| PKG-06 | `std` is embedded and its prelude is implicit | 7 | T7 |
| PKG-07 | Reflection reaches crates | 7 | T8, T12 |
| PKG-08 | A multi-crate design serializes and is validated | 4 | T13 |
| PKG-09 | The reference docs describe crates | 3 | T14 |
| PKG-10 | A manifest is read and validated | 13 | T16, T17, T18 |
| PKG-11 | Path dependencies become a crate graph | 11 | T19, T20, T21 |
| PKG-12 | Git dependencies | 6 | T22 |
| PKG-13 | The lockfile pins git packages | 9 | T23, T24 |
| PKG-14 | One call loads a project | 4 | T15, T25 |
| PKG-15 | `piperine-project` is manifest, dependencies and lockfile only | 3 | T15, T25 |

### User decision to requirement

| Decision | Requirement |
| -------- | ----------- |
| U1 no search paths, Rust's model | PKG-01, PKG-02 AC 7, PKG-03, PKG-05 |
| U2 `std` embedded | PKG-06, PKG-04 AC 2, PKG-05 AC 8 |
| U3 `piperine-project` cut down | PKG-10 to PKG-15 |
| U4 `msl-stdlib` next | Out of Scope (the library's content) |
| U5 roles | `tasks.md` (batches, Verifier) |
| U6 name clash: Rust 2018 exactly | PKG-01 AC 3 and 10, PKG-05 AC 5 to 7 |

---

## Sources

- `.specs/features/msl-packages/context.md` - the user's decisions U1 to U6 (2026-11-14)
- The orchestrator's brief for this feature - what to specify (spec text, POM, `piperine-project` port), the build rules and the task shape
- `docs/spec/` - the specification this feature amends (§0.3, §0.8, §0.9, §3.4, Appendix B.3.6, Appendix G); `.specs/features/msl-spec/STYLE.md` - how it is written
- `.specs/STATE.md` MD-54 (the POM is built once; only lowering creates nodes), MD-55 (`.msl`, Rust's `mod` layout), MD-56 (D3 core names, D5 `E0162`), MD-57 (a spec edit lands where its guard can pass)
- `CLAUDE.md`, "POM navigability mirrors the source" - the rule PKG-04 AC 6 holds the design to
- `crates/piperine-lang/src/pom/`, `docs/pom/` - the object model this feature extends
- `crates/piperine-project/src/` - the crate this feature cuts down and ports
- *The Rust Reference* (edition 2024), "Names: preludes" and "Paths", and *The Cargo Book*, "Specifying dependencies" - the rules §3.5 and §3.6 follow

---

## Success Criteria

- [ ] All 117 checks of `tasks.md` are green at `HEAD` (`check_proofs.py msl-packages`).
- [ ] `cargo test -p piperine-lang` and `cargo test -p piperine-project` report 0 failed, with no test removed and no `#[ignore]`.
- [ ] Both crates build all targets with no warning.
- [ ] A package `app` with a path dependency `util` loads with `load_project` into a design with zero diagnostics whose instance links into `util`.
- [ ] No test contacts the network: every git repository of the suite is created in a temporary directory.

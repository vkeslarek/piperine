# CLI Packages and Outputs Specification

**Approved**: 2026-11-18 - user: "Sobre as open questions, vou de aceito todas ok?" (every open question takes its recommended option; exceptions noted where the user decided otherwise)

**Size:** M, about 26 tasks (`GAPS.md` sizes each item S). Part of the host-iii group (`.specs/features/host-iii/spec.md` holds the group's "Gaps closed" table).

## Questions answered (user, 2026-11-18: every recommendation accepted - "Sobre as open questions, vou de aceito todas ok?")

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | "Plugin scripts" (H10): the plugin system was removed (user, `solver-restructure`: "plugin pro enquanto vai embora"). What closes the item? | (a) external subcommands, as Cargo does: `piperine <name> [args]` with `<name>` not a built-in command runs `piperine-<name>` found on `PATH`, with the project root in `PIPERINE_ROOT`; (b) scripts declared by a package: `[scripts] <name> = "<command>"` in `Piperine.toml`, run by `piperine x <name>`; (c) retire the item with the plugin system: `GAPS.md` drops it, citing the user's removal | **(a)**. It needs no plugin system, no manifest key and no trust model; a team ships a tool as a binary. With (b), CLP-08 is rewritten around the table; with (c), CLP-08 is dropped and H10's row is closed by the decision |
| Q2 | The REPL (H10) | (a) `piperine repl`: a line shell over a session with the commands `open <run>`, `set <path>=<value>`, `run`, `continue <stop>`, `print <path>`, `info`, `quit`, each a call of `piperine-api`; (b) the REPL is Python's: `piperine repl` starts Python with `piperine` imported and the project opened (needs `python-host`); (c) none: the item is closed by `python-host`'s interactive use | **(b)** if `python-host` is approved, else **(a)**. A second command language duplicates a host language; Python's REPL is the one users know. With (a), CLP-09 holds; with (b), CLP-09 becomes a launcher criterion and moves after `python-host` |
|  | **Resolved (user, 2026-11-18): (a), the Rust line shell.** User, 2026-11-18, verbatim: "Eu acho que podemos fazer tudo em RUST por enquanto e DEPOIS pensamos em integrar python. Senão cada mod da api do host vai tomar mais tempo ainda". A Python REPL comes with `python-host`, later. |  |  |
| Q3 | How does a run ask for a PNG or a terminal plot? | (a) the `plot` path's extension: `.svg` (today) or `.png`; `plot = "-"` (or `--plot -`) draws a text plot on standard output; (b) a separate key `plot_format` | **(a)**. One key, the format read from what the user wrote |

## Problem Statement

`piperine` has seven commands (`check`, `build`, `run`, `test`, `new`, `clean`, `fmt`). A dependency is added by editing `Piperine.toml` by hand; there is no way to see the dependency tree, update a locked dependency, refuse a lockfile change in CI (`--locked`) or build without the network (`--offline`), although `piperine-project` resolves git dependencies and writes a lockfile (MD-58, MD-59). A run writes text, CSV, JSON Lines and SVG; it cannot write ngspice's raw format (so ngspice's and other waveform viewers cannot open it), nor a PNG or a terminal plot. Plugin scripts and a REPL existed in the PHDL era and left with the plugin system and Python (`GAPS.md` H10).

## Goals

- [ ] `piperine add`, `remove`, `tree` and `update` manage dependencies as `cargo` does, keeping the manifest's comments.
- [ ] `--locked` and `--offline` make a build reproducible and network-free, loud when they cannot.
- [ ] A run writes an ngspice raw file that ngspice itself loads back with the same values.
- [ ] A run draws a PNG or a terminal plot.
- [ ] Plugin scripts and the REPL are closed by the user's answers to Q1 and Q2.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| A package registry (`piperine publish`, versions resolved by semver from an index) | Not in `GAPS.md`; dependencies are path and git (MD-58) |
| The plugin system (trust, permissions, device plugins) | Removed by the user; Q1 (a) needs none |
| Python itself | `python-host` |
| A raw file of an `sp`, `pz` or `tf` result | The raw format's plots are vectors over a scale; those results are tables of another shape. Refused naming the analysis |
| Interactive plot windows | A file or the terminal only |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Plugin scripts | external subcommands (Q1 (a)) | Q1 recommendation | n - open (Q1) |
| REPL | Python's, via `python-host` (Q2 (b)), else a line shell | Q2 recommendation | n - open (Q2) |
| Plot formats | by the `plot` path's extension, `-` for the terminal (Q3 (a)) | Q3 recommendation | n - open (Q3) |
| Editing the manifest | a format-preserving TOML editor: comments, order and spacing of untouched lines stay byte-identical | a user's manifest is source; `cargo add` behaves so | n - planner |
| `add` sources | `--git <url>` with one of `--rev`, `--tag`, `--branch`, or `--path <dir>`; the name defaults to the dependency's `[package] name` | the two sources MD-58 has | n - planner |
| `--locked` | fails when the lockfile is missing or would change, before any write | Cargo's meaning | n - planner |
| `--offline` | no network call; a git dependency is taken from the cache at its locked commit; fails naming the dependency when it is not cached | Cargo's meaning | n - planner |
| Raw format | ngspice's binary raw by default; `raw_format = "ascii"` for the text form; the run key `raw` with the flag `--raw` | ngspice writes binary by default; both are documented by ngspice | n - planner |
| A discrete signal in a raw file | a `bool` or integer carrier written as a real vector of its value, unknown as NaN; a carrier of another type refused naming it | the raw format has no digital type; a silent drop would lose a printed signal | n - planner |

**Open questions:** none - answered 2026-11-18, every recommendation accepted (see the top of this file).

**Implicit-requirement dimensions.** Input validation and bounds: CLP-01 AC 3 to 5, CLP-02 AC 2, CLP-07 AC 4. Failure and partial-failure states: CLP-01 AC 6 (a failed resolve leaves the manifest unchanged). Idempotency and retry: CLP-03 AC 2 (`update` with nothing new changes nothing). Auth and rate limits: N/A, git auth is the user's git configuration. Concurrency and ordering: N/A, one command at a time. Data lifecycle: CLP-04 (the lockfile). Observability: CLP-03 AC 1 (each updated dependency printed). External-dependency failure: CLP-05 (network down, `--offline`), CLP-07 AC 5 (ngspice absent in the oracle). State-transition integrity: CLP-01 AC 6.

---

## User Stories

### P1: Add and remove a dependency ⭐ MVP

**User Story**: As a package author, I want `piperine add` and `piperine remove`, so that I never hand-edit `Piperine.toml` for a dependency.

**Why P1**: H10.

**Acceptance Criteria**:

1. WHEN `piperine add --git <url> --tag <tag>` runs in a package THEN the system SHALL add the dependency to `[dependencies]`, resolve it, update the lockfile and exit 0 (CLP-01)
2. WHEN `piperine add --path <dir>` runs THEN the system SHALL add a path dependency named by the target's `[package] name` (CLP-01)
3. IF the dependency's name is already in `[dependencies]` THEN the system SHALL exit 1 naming it and change nothing (CLP-01)
4. IF `--git` is given with none or more than one of `--rev`, `--tag`, `--branch` THEN the system SHALL exit 2 naming the flags (CLP-01)
5. IF `--path` names a directory with no `Piperine.toml` THEN the system SHALL exit 1 with the package error the resolver gives (CLP-01)
6. IF the resolution fails THEN the system SHALL leave `Piperine.toml` and the lockfile byte-identical and exit 1 with the resolver's error (CLP-01)
7. The system SHALL keep every comment and every untouched line of `Piperine.toml` byte-identical when it adds or removes a dependency (CLP-01)
8. WHEN `piperine remove <name>` runs THEN the system SHALL remove the dependency from `[dependencies]` and its entries from the lockfile, and exit 0 (CLP-02)
9. IF `<name>` is not a dependency THEN the system SHALL exit 1 naming it and listing the dependencies (CLP-02)

**Independent Test**: in a copy of `examples/buck`, `piperine add --path ../rc` then `piperine remove rc` leaves `Piperine.toml` byte-identical to the start.

---

### P1: See and update the dependency tree ⭐ MVP

**User Story**: As a package author, I want `piperine tree` and `piperine update`, so that I see what I depend on and move it forward on purpose.

**Why P1**: H10.

**Acceptance Criteria**:

1. WHEN `piperine tree` runs THEN the system SHALL print the package, then each dependency indented under its dependent with its version and source (path, or git URL and locked commit), a dependency seen twice marked `(*)` after its first listing (CLP-03)
2. WHEN `piperine update` runs THEN the system SHALL re-resolve every git dependency to the current commit of its `branch` or `tag`, rewrite the lockfile, and print one line per dependency whose commit changed (CLP-03)
3. WHEN `piperine update <name>` runs THEN the system SHALL update that dependency only (CLP-03)
4. WHEN nothing changed upstream THEN `piperine update` SHALL leave the lockfile byte-identical and print nothing (CLP-03)
5. WHEN a dependency is pinned by `rev` THEN `piperine update` SHALL leave it at that commit (CLP-04)

**Independent Test**: a package with a git dependency on a local bare repository; a new commit on its branch, then `piperine update`, moves the lockfile to it.

---

### P1: Reproducible and offline builds ⭐ MVP

**User Story**: As a CI owner, I want `--locked` and `--offline` on every command that resolves, so that a build uses exactly the lockfile and no network.

**Why P1**: H10.

**Acceptance Criteria**:

1. WHERE `--locked` is given to `check`, `build`, `run` or `test` the system SHALL fail with exit 1, naming the dependency, when resolving would change or create the lockfile, and write nothing (CLP-05)
2. WHERE `--offline` is given the system SHALL make no network access, and take each git dependency from the cache at its locked commit (CLP-05)
3. IF `--offline` is given and a git dependency's locked commit is not in the cache THEN the system SHALL fail with exit 1 naming the dependency and the commit (CLP-05)
4. IF `--offline` is given and a git dependency has no lockfile entry THEN the system SHALL fail with exit 1 naming the dependency (CLP-05)

**Independent Test**: with the cache filled and the git URL made unreachable, `piperine build --offline --locked` succeeds; with the cache cleared it fails naming the dependency.

---

### P1: ngspice raw output ⭐ MVP

**User Story**: As a designer, I want a run to write ngspice's raw format, so that I open it in the viewers I already use.

**Why P1**: H10; the raw file is ngspice's interchange.

**Acceptance Criteria**:

1. WHEN a run gives `raw = "<file>"` (or `--raw <file>`) THEN the system SHALL write the run's table as one ngspice raw plot: title, date, plot name, flags `real` or `complex`, the number of variables and points, the variables with their types (`time`, `frequency`, `voltage`, `current`), then the values (CLP-06)
2. WHEN the run is `ac` THEN the system SHALL write complex values and the type `frequency` for the scale (CLP-06)
3. WHERE `raw_format = "ascii"` is given the system SHALL write ngspice's text raw form; otherwise its binary form (CLP-06)
4. IF `raw` is given to an `sp`, `pz`, `tf` or `sens` run, or a printed signal is a carrier of a type other than `bool` or an integer THEN the system SHALL report `P0010` naming the key and the analysis or the signal (CLP-07)
5. WHEN ngspice loads the raw file written by each of the 7 twins and prints every vector THEN each value SHALL equal the run's table's value exactly in the binary form and under the contract of its domain in the text form; WHERE ngspice is absent the test SHALL print the harness's skip line (CLP-07)

**Independent Test**: `piperine run tests/ngspice/tran_rc.msl --raw out.raw`, then `ngspice -b` with `load out.raw` and `print v(out)`.

---

### P2: PNG and terminal plots

**User Story**: As a user at a terminal or writing a report, I want a PNG or a text plot.

**Why P2**: H10.

**Acceptance Criteria**:

1. WHEN `plot` names a path ending in `.png` THEN the system SHALL write a PNG image of the plot it draws as SVG today, same series, axes and labels (CLP-08)
2. WHEN `plot` is `-` THEN the system SHALL draw the plot as text on standard output after the table, at most 100 columns wide (CLP-08)
3. IF `plot` names a path with an extension other than `.svg` and `.png` THEN the system SHALL report `P0010` naming the key and the two extensions (CLP-08)

**Independent Test**: `piperine run tran_rc --plot out.png` writes a PNG whose decoded size is the SVG's.

---

### P2: Plugin scripts (Q1 (a))

**User Story**: As a team, I want to add our own commands to `piperine`, so that our tools sit beside the built-in ones.

**Why P2**: H10.

**Acceptance Criteria**:

1. WHEN `piperine <name> [args]` names no built-in command and `piperine-<name>` is on `PATH` THEN the system SHALL run it with the arguments, with `PIPERINE_ROOT` set to the package root when there is one, and exit with its exit code (CLP-09)
2. IF neither a built-in command nor `piperine-<name>` exists THEN the system SHALL exit 2 naming `<name>` and listing the built-in commands (CLP-09)
3. WHEN `piperine --list` runs THEN the system SHALL list the built-in commands and every `piperine-*` on `PATH` (CLP-09)

**Independent Test**: a script `piperine-hello` on `PATH` is run by `piperine hello world`.

---

### P2: The REPL (Q2)

**User Story**: As a user exploring a design, I want an interactive prompt on a session.

**Why P2**: H10.

**Acceptance Criteria**:

1. WHERE `python-host` is installed (Q2 (b)) WHEN `piperine repl [<run>]` runs THEN the system SHALL start an interactive Python with `piperine` imported and a session of the run opened as `sim` (CLP-10)
2. IF Python or the `piperine` Python package is not installed THEN `piperine repl` SHALL exit 1 naming what is missing and how to install it (CLP-10)

**Independent Test**: `echo 'print(sim.top)' | piperine repl` in `examples/rc` prints the top.

---

### P3: The documents

**User Story**: As a reader of the Book and `GAPS.md`, I want the new commands documented.

**Why P3**: no behavior depends on it.

**Acceptance Criteria**:

1. WHEN this feature merges THEN the Book's chapter "The command line" SHALL show `add`, `remove`, `tree`, `update`, `--locked`, `--offline`, `raw` and a terminal plot, each a listing held to the binary (CLP-11)
2. WHEN this feature merges THEN `GAPS.md` SHALL no longer list H10, and `piperine --help` SHALL list the new commands (CLP-12)

**Independent Test**: `cargo test -p piperine-cli --test book --test docs`.

---

## Edge Cases

- WHEN `piperine add` runs outside a package THEN the system SHALL exit 1 naming the missing `Piperine.toml`.
- WHEN `piperine tree` runs in a package with no dependency THEN the system SHALL print the package line only.
- IF a raw file's directory does not exist THEN the system SHALL exit 1 naming the path.
- WHEN a terminal plot has more series than 8 THEN the system SHALL draw the first 8 and print one line naming the rest.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| CLP-01 | P1: Add and remove | - | Pending |
| CLP-02 | P1: Add and remove | - | Pending |
| CLP-03 | P1: Tree and update | - | Pending |
| CLP-04 | P1: Tree and update | - | Pending |
| CLP-05 | P1: Reproducible and offline | - | Pending |
| CLP-06 | P1: ngspice raw output | - | Pending |
| CLP-07 | P1: ngspice raw output | - | Pending |
| CLP-08 | P2: PNG and terminal plots | - | Pending |
| CLP-09 | P2: Plugin scripts | - | Pending |
| CLP-10 | P2: The REPL | - | Pending |
| CLP-11 | P3: The documents | - | Pending |
| CLP-12 | P3: The documents | - | Pending |

**Coverage:** 12 total, 0 mapped to tasks (spec only).

## Gaps closed

| Gap | What | Requirements |
| --- | ---- | ------------ |
| H10 | `add` | CLP-01 |
| H10 | `remove` | CLP-02 |
| H10 | `tree` | CLP-03 |
| H10 | `update` | CLP-03, CLP-04 |
| H10 | `--locked` | CLP-05 |
| H10 | `--offline` | CLP-05 |
| H10 | plugin scripts | CLP-09 (Q1 (a)); with Q1 (c), closed by the user's removal of plugins |
| H10 | a REPL | CLP-10 (Q2 (b), after `python-host`); with Q2 (a), a line shell |
| H10 | ngspice raw output | CLP-06, CLP-07 |
| H10 | a terminal or PNG plot | CLP-08 |

## Dependencies

| Depends on | For |
| ---------- | --- |
| `piperine-project` (MD-58, MD-59) | resolution, the cache under `deps`, the lockfile |
| `python-host` (spec) | CLP-10 under Q2 (b) only |
| `host-iii` | `continue` in the line shell under Q2 (a) only |

## Sources

- `GAPS.md` H10; the coverage map of 2026-11-17.
- `.specs/STATE.md` MD-58, MD-59, MD-65; the `solver-restructure` decision removing plugins and Python.
- `crates/piperine-project/src/{manifest,resolver,lockfile,git}.rs`; `crates/piperine-cli/src/render/`.
- ngspice manual, chapter on the raw file format (`write`, `load`).

---

## Success Criteria

- [ ] `add` then `remove` leaves a manifest byte-identical.
- [ ] ngspice loads every twin's raw file back with the same values.
- [ ] `--offline --locked` builds `examples/buck` with no network.

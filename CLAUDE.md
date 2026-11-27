# Piperine — Claude Code Instructions

Piperine is a mixed-signal language (MSL) and its simulator. Source in MSL is parsed, checked and
elaborated into an IR; the IR is printed as Rust and built into a device library that speaks one
device ABI; a native solver (an owner over an analog and a digital sub-solver) runs it. Rust
workspace, edition 2024. `docs/spec/` is the only grammar and semantics source.

## Pipeline

```
MSL source (.msl) + embedded std (crates/piperine-lang/std/*.msl)
  │ piperine-lang   msl/ parse ─► pom/ Design (the source, resolved) ─► types/ check ─► elab/ elaborate
  ▼
Ir                  piperine-ir     arenas + typed ids, check, printer/parser
  │                 piperine-plan   BodyPlan (ABI shape + function table), Netlist::circuit
  ▼
device library      piperine-transpile  IR ─► Rust ─► rustc ─► cdylib exporting piperine_abi_1 (cached)
                    piperine-interp     reference interpreter over the same plan (differential oracle)
  ▼
piperine-abi        the device ABI (#[repr(C)] contract, Device trait) — the only device contract
  ▼
piperine-solver     owner: analyses + owner protocol ─┬─ piperine-analog-solver  (MNA, Newton, TR-BDF2)
                                                     └─ piperine-digital-solver (byte nets, events)
  ▼
piperine-api        Project, Session, params, probes, records ◄── piperine-cli (`piperine`)
```

Dependency arrows are part of the design — breaking one is a regression:
- `piperine-abi`, `piperine-ir` and `piperine-numbers` are leaf crates (no in-tree dependency).
- The sub-solvers depend only on `piperine-abi` and never on each other or on the owner; analog
  and digital meet only through events at an instant, run by the owner protocol.
- Generated device code links only `piperine-abi` and the embedded runtime (`rt/`, which embeds
  `piperine-numbers` as source).

## UNBREAKABLE RULE — the POM mirrors the source

The POM (`piperine-lang/src/pom/`) is what the author wrote, every name resolved. Resolution and
elaboration only *add* links or *validate*; they never rewrite authored structure. The elaborator
reads the POM and never writes it. Anything collapsed, flattened, hoisted or monomorphized lives
in the IR or in a separate side table, never in the authored module/instance tree.

## Crates

| Crate | Role |
|---|---|
| `piperine-lang` | Front end: `msl/` (lexer, parser, printer, formatter), `pom/` (the object model + reflection), `types/` (checker), `elab/` (POM → `Ir`), `std/` (the standard library, in MSL). |
| `piperine-ir` | The IR object model, builder, `Ir::check`, printer and parser. |
| `piperine-plan` | What both backends share: a body's ABI shape and function table; the design's circuit. |
| `piperine-transpile` | Compiled backend: IR printed as Rust, built by `rustc`, cached by hash, loaded. |
| `piperine-interp` | Reference interpreter over the same plan; a differential oracle for the transpiler. |
| `piperine-abi` | Device ABI: raw layout, shapes, `Device` trait, thunks, `caller::Handle`, partials verification. `unsafe` only in `raw.rs`, `thunk.rs`, `caller.rs`. |
| `piperine-numbers` | Integers of any width, exact `f16`, correctly rounded `f128`; also embedded as runtime source. |
| `piperine-solver` | Owner: analyses (op/dc/ac/tran/noise/…), owner protocol, device binding, `Net` naming. |
| `piperine-analog-solver` | MNA + linear algebra, `ConvergencePlan` (gmin/source stepping, Newton), `TrBdf2` + LTE, device assembly. |
| `piperine-digital-solver` | Byte-net event engine: commit queue, levelized order, instants, time advance. |
| `piperine-api` | The host library: `Project`, `Session`, `Params`, `Probe`, `Record`, `Verdict`, `Failure`. Every rule of what a run prints or judges lives here. Root package `piperine` re-exports it. |
| `piperine-cli` | The `piperine` binary, a thin shell over `piperine-api`. |
| `piperine-project` | `Piperine.toml`, dependencies, lockfile. |
| `piperine-parity` | Test-only: ngspice parity of the MSL twins under one `ToleranceContract`, runs MSL from source, the buck targets. |
| `piperine-device-fixtures` | Test-only: the hand-written ABI devices the back-end test suites share, one type per device, and `PartialsCheck`. Depends on `piperine-abi` alone. |
| `piperine-repository-guards` | Test-only: the guards over the repository itself (workspace boundaries, suite hygiene, the proof ledger) and the source scan they share. |

## Build and test

```sh
cargo build                                   # default members; zero warnings is the bar
cargo test -p <crate>                         # the crates you touched
cargo test -p <crate> --test <target>         # one target
cargo test -p piperine --test integrated      # small mixed-signal designs end to end (~30 s)
```

- The whole workspace builds: `cargo build --workspace --all-targets`. The full test run is
  `cargo test --workspace`, once per feature (gate policy below).
- **Gate policy:** per task and per batch, build + the touched crates/targets only. The full
  suite (all packages, `piperine-parity --nocapture` with ngspice, the buck targets, the twins)
  runs **once per feature**, at final verification before merge.
- The buck targets are `test = false`; run them by name (`buck_*`, `book_buck`, `buck_scenarios`)
  as listed in `examples/buck/README.md`.
- Never retune a tolerance. ngspice parity first; `ToleranceContract` is the only place a
  numeric tolerance is written.
- Build hygiene: set `CARGO_TARGET_DIR` per worktree (never share one between concurrent builds),
  `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0` keep the target dir small. Watch disk space.

## Code rules (binding)

1. **Code follows the spec.** A divergence from `docs/spec/` or the Book is a defect to fix, or
   a spec amendment the user approves — never an accepted documented state.
2. **Fail loud.** A form not supported yet is refused by name (`P0001`, or `Unsupported` in the
   transpiler), with an owner in `GAPS.md`. Never a silent `0.0`, no-op or fallback.
3. **No free functions** (tests included): every function is a method of a type or trait that
   owns it. Exceptions only where the language requires one (`main`, `#[test]`, ABI exports).
4. **Names say what things do:** full words, no abbreviations, object-oriented naming; one
   purpose per type; files named for their system role (never `utils.rs`).
5. **No god methods;** small methods, flat control flow. No file-scope lint suppression,
   no `#[allow]` to hide a design problem (`too_many_arguments` → a parameter object).
6. **Doc comments on every item.**
7. **No macros** (`macro_rules!`, proc-macro codegen): data tables and methods instead.
8. **No language special cases:** builtins follow the same rules as user items and are
   declared in the MSL prelude (`std/`); no hidden peephole for one spelling.
9. **No `unwrap`/`expect` on user-input paths.**

## Tests

- No hand-kept list. Enumerate with `ls crates/*/tests/*.rs tests/` or the `Running` lines of
  `cargo test`; every target opens with a `//!` scope header.
- A target lives in the crate it exercises. Root `tests/` holds only the integrated tests:
  `tests/book/` (one target per Book chapter + `inventory.tsv`), `tests/integrated/`,
  `tests/scenarios.rs`, `tests/buck_scenarios.rs`, fixtures in `tests/e2e/`.
- `crates/piperine-repository-guards/tests/suite/suite_hygiene.rs` guards the suite: no switched-off code, no
  `#[ignore]`, no target without `//!`, no file-scope lint suppression, no dead identifiers.
- Never weaken an assertion to make a test pass. Re-pins are exact.
- Frozen corpora: `tests/fixtures*`, the twin goldens. Book/golden numbers may move after a
  solver change only while they stay within ngspice/analytic tolerance.

## Documentation and tracking

- `docs/spec/` — the MSL specification (Parts I–VIII, appendices A–H; `index.md` is the map).
- `docs/book/` — the Book (`content.html`, built by `build.py`). Every listing is held by a test
  in `tests/book/` and a row in `tests/book/inventory.tsv` (runs / holds / refused + owner).
- `GAPS.md` — what is refused or wrong today (L-numbers, each with an owner).
- `ROADMAP.md` — what is planned, in order.
- `.specs/STATE.md` (decision log, MD-numbers) + `.specs/features/<name>/` (spec, design, tasks,
  verification, proofs) — the spec-driven workflow: an author never verifies their own work.
- `docs/pom/` — the POM guide for tool authors.

## Git

- Commits: conventional type (`feat|fix|docs|test|refactor|…`; `merge` is not a type), one task
  per commit, trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Never `git stash`. Work in a worktree per lane; remove it when done (push the branch first).

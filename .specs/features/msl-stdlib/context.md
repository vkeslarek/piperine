# msl-stdlib Context

**Gathered:** 2026-11-14
**Spec:** `.specs/features/msl-stdlib/spec.md`
**Status:** Q1 to Q5 answered (see "Answers"); one consequence of Q2 awaits the user (see "Consequences of the answers")

---

## Feature Boundary

Ship the standard library of MSL as real source (`crates/piperine-lang/std/*.msl`), make it the normative text of Part VII (`docs/spec/part_vii_stdlib.md`), and prove with tests that the galleries of Appendices D, E and F build clean against it. The front end stops at parse and POM resolution: nothing here type-checks, elaborates or simulates.

---

## Implementation Decisions

### Decisions already taken (binding)

| # | Decision | Source |
| - | -------- | ------ |
| D4 | The standard library is a normative source: ship `std/*.msl`, Part VII's fences are excerpts of it, `pub` on every export, and "the galleries build clean" is a gate | user, 2026-11-14: "sim" (`.specs/features/msl-audit/report.md`, decision D4, findings A-03 and A-42; `STATE.md` MD-56) |
| D3 | `gnd`, `Nature`, `m`, `dtemp`, `str` and the core primitives of §14 are core names: no import, no declaration, not in `std` | user, 2026-11-14: "sim" (MD-56) |
| U1 | 100% Rust/Cargo model: a crate is reached only through a named dependency; no search path | user: "-I é ruim demais, faremos modelo RUST 100%." (`msl-packages/context.md`) |
| U2 | `std` is a crate embedded in the binary, inside `piperine-lang`, its files compiled in with `include_str!` | user: "std fica embutida o binário." (MD-59) |
| Prelude | The prelude is the `pub` items of `std::prelude`, implied in every file of every crate, `std`'s files included | `docs/spec` §3.4.2, §3.5 rules 3 and 9 (`msl-packages`, PKG-06) |
| Roles | Sonnet implements, Opus verifies | user, standing rule |
| Rules | The POM mirrors the source (CLAUDE.md, MD-54); no macros of our own; no `CARGO_TARGET_DIR` override; `cargo test -p <crate>`, never `--workspace` | project rules |

### The MSL Book (user requirement, added while planning)

- User, 2026-11-14: "Bora colocar na spec do std lib para atualizar o book, criar uma session sobre o que tem no core + std lib".
- "The MSL Book" is the user's living, reader-friendly manual, published as the artifact `https://claude.ai/artifact/6nvqf1rKtF3TteqjEv6z26`. It gains a chapter "Core and standard library": what is core (the §14 primitives, the core names `gnd`, `Nature`, `m`, `dtemp`, `str`, the `!` forms) and what is in `std`, namespace by namespace, with what `std::prelude` brings in, short plain-language descriptions and small examples. Chapter 9's packages note and the cheat sheet are refreshed where the library changes them.
- The Book's source lives only in a session scratchpad today (`manual/build/{content.html,template.html,build.py}`, output `../index.html`). The feature moves it into the repository so it is durable and testable.
- Publishing the artifact is done by the orchestrator after the merge, not by a task.
- Planner choices inside the requirement: location `docs/book/`; the built `index.html` is not committed; the new chapter is chapter 10 (id `core-std`), after "Files, modules and packages"; checks are a `piperine-lang` test target (`book`).

### Facts measured while planning (2026-11-14, a throwaway probe, not committed)

- Built against the embedded (empty) `std`, 0 of the 18 gallery fences are clean: 250 `E0161`.
- Built against Part VII's own `rust` fences with `pub` added, plus the nine models the galleries take from the normative parts, 18 of 18 report zero diagnostics.
- The same text as a library reports: 13 `E0125` (the functions Part VII gives as a signature only), 1 `E0161` (`Self::WIDTH` in `trait Known: Repr`), and it leaves every `known`, `known_or` and `is_known` call on a carrier `Unresolved` with no diagnostic (the blanket `impl<T: Repr> Known for T {}` is not searched).
- Seven modules are "head and params only" in Part VII (`zi_np`, `zi_zd`, `zi_zp`, `laplace_nd`, `laplace_np`, `laplace_zd`, `laplace_zp`); `noise_table` is the eighth and has a one-line definition in prose.
- The module `Tran` (§22.3.7) and the analysis kind `Tran` (§11.1.1) share a name. The prelude is looked up before the core names (§3.5 rule 4), so with a real `std` every `analysis(Tran)` silently names the pass switch.

### Agent's Discretion

- File layout inside `std` (one file per §22.x section), the guard design, the gallery support fixture, which usage examples of Part VII become `rust,fragment`.

### Declined / Undiscussed Gray Areas → Assumptions

None declined. Q1 to Q5 are not answered yet; each is in the spec's Assumptions table with `n`.

---

## Open Questions (user)

### Q1. How does `std` declare an item the engine implements?

Part VII gives 9 functions as a signature and a contract (§22 rule 4 already allows that), and 7 modules as a head and params. A `.msl` file cannot hold them today: a function without a body is `E0125` unless it is `extern "C"`, and `extern "C"` takes scalars only.

| Option | What it is | Cost |
| ------ | ---------- | ---- |
| **A (recommended)** | A second ABI string, `extern "intrinsic"`, on `fn` and on `module`, accepted only inside the crate `std`. `pub extern "intrinsic" fn pnjlim(new: f64, old: f64, nvt: f64, vcrit: f64) -> f64;` | no grammar change (B.3.2 already takes `extern STRING` on both), no new construct in B.1, no new code; §3 rule 10 and §8.5 gain one sentence each. Rust has the same form (`extern "rust-intrinsic"`) |
| B | Move those items to §14: they become core names, not `std` | no new ABI, but §14 grows by 16 names, they stop being replaceable (§22 rule 3) and §14.13 ("not core") is reversed for `table_model`, `laplace_*`, `zi_*` |
| C | Write all of them in MSL now | needs `str` byte access in Appendix H and const-generic arithmetic (`[f64; N + 1]`), and makes numerics nobody can run yet (spline, `pnjlim`, state-space forms) normative |

Recommendation: A. It reuses a form the language has, keeps §14 as it is, and keeps "signature and contract" items replaceable.

### Q2. Which items are written in MSL now, and which are intrinsic?

Recommendation: write in MSL the five whose definition Part VII already states in prose (`same`, `deltalim`, `db10`, `db20`, `noise_table`). Declare `extern "intrinsic"` the rest: `matches_bits`, `logic`, `pnjlim`, `fetlim`, `limvds`, `table_model`, `normal`, `parse_table`, `parse_hex`, and the modules `zi_np`, `zi_zd`, `zi_zp`, `laplace_nd`, `laplace_np`, `laplace_zd`, `laplace_zp`. Reason: no evaluator exists, so a new body could not be run; a body that was never run should not become normative text. A later feature replaces an intrinsic by an MSL body once it can be executed.

Alternative: also write `pnjlim`, `fetlim`, `limvds` (ported from ngspice) and `normal` (a fixed generator, which would close the `normal` half of G.64).

### Q3. `Tran` names two things

Recommendation: rename the pass switch of §22.3.7 to `TranIf` (it has an enable, as Verilog's `tranif1`), keep `Tran` for the analysis kind, and add a guard that no `std` export has the name of a core row. Alternatives: rename it `Pass`; or keep both and require `Analysis::Tran` wherever the analysis kind is meant (touches §11, §13, §16, §22 and every gallery).

### Q4. Public namespaces of `std`

Recommendation: one `pub` namespace per §22.x section, plus two for what Part VII cites from elsewhere: `std::constants`, `std::carriers`, `std::values`, `std::analog`, `std::nets`, `std::convert`, `std::monitor`, `std::io`; `std::prelude` re-exports every `pub` item of all of them (§22 rule 1). The names are user-visible (`use std::nets::Wire;` works); the user may prefer others.

### Q5. The prelude constants

§1 rule 3 says the prelude has `Q`, `K_B`, `EPS0`, `PI`, `E` and `inf`; today they are rows of the built-in table. Recommendation: move all six to `std::constants` as `pub const` items, `inf` written `1.0 / 0.0` (a float literal out of range is `E0909`, and `f64::INFINITY` is not in Appendix H). Alternative: keep `inf` as a core name and move the other five.

---

## Specific References

- The audit's fix for A-03: "extract the stdlib into real files, `pub` on every exported item, generated from or checked against the Part VII fences; add a test: every gallery fence of App. D/E/F builds with zero diagnostics against it (models referenced across fences supplied by a small table)".
- G.38 (Appendix G): "They are to become `std/*.msl` sources that compile with zero diagnostics, and Part VII's fences excerpts of them".
- MD-57: a spec edit lands where its guard can pass; `spec_text` guards the wording.

---

## Deferred Ideas

- MSL bodies for the intrinsic items, once an evaluator can run them (closes the rest of §14.13's "written in the language").
- A device library (`Resistor`, `Capacitor`, `Switch`, the SPICE models) as a crate of its own. The galleries take these models from the normative parts; they are not Part VII items.
- The fences of Parts I to VI built clean against `std` (A-03 counted 46 of 95 with `E0161`). Most are pieces that need a context table of their own.
- `str` methods in Appendix H (byte access), which would let `logic` and `matches_bits` be written in MSL.
- Binding each intrinsic to an engine implementation (elaborator and codegen do not exist yet).

## Answers (user, 2026-11-14)

| Q | Answer (user's choice, verbatim option) | Effect |
| - | --------------------------------------- | ------ |
| Q1 | "extern \"intrinsic\" (Recommended)" | `extern "intrinsic"` on `fn` and `module`, legal only in crate `std` |
| Q2 | "As many as possible" | every std item expressible in MSL today gets an MSL body, even though nothing executes bodies yet; only what MSL cannot express stays `extern "intrinsic"` |
| Q3 | "Rename the analysis kind" | the std module keeps the name `Tran`; the analysis kind `Tran` is renamed; plus a guard that no std export equals a core name |
| Q4+Q5 | "Yes, both (Recommended)" | namespaces `std::{constants, carriers, values, analog, nets, convert, monitor, io}`, all re-exported by `std::prelude`; `Q`, `K_B`, `EPS0`, `PI`, `E`, `inf` move to `std::constants` (`inf` = `1.0 / 0.0`) |

## Consequences of the answers (planner, 2026-11-14)

Choices made inside the answers. Each is a row of the spec's Assumptions table.

| # | Consequence | Choice |
| - | ----------- | ------ |
| Q3 | the new name of the analysis kind | `Transient`: a whole word, as `Noise` and `Static` are, and the word Parts V and VIII already use in prose |
| Q2 | the intrinsic set, item by item (`design.md`, "The split") | five functions, the ones that read the characters of a `str`: `matches_bits`, `logic`, `table_model`, `parse_table`, `parse_hex`. The other 16 items get MSL bodies |
| Q1 + Q2 | `extern "intrinsic"` on a module | not specified now: after Q2 no module needs it |
| Q2 | `E0707` (a complex root without its conjugate, a zero denominator) | removed; both are a false `where` of the param, `E0317`. An MSL body cannot raise a code of its own |
| Q2 | the generator of `normal` | SplitMix64 for two uniform draws and Box-Muller; the source in `std::analog` is its documentation (closes the `normal` half of G.64) |
| Q2 | `pnjlim`, `fetlim`, `limvds` | transliterated from `crates/piperine-ir/src/limit.rs` |

**Awaiting the user.** `laplace_nd` as written requires `D >= 1` and `N <= D` (a proper transfer function), which is narrower than Part VII's contract today. Recommendation: accept it and state it as a rule of §22.2.14 (`E0304`); an improper `H(s)` differentiates the input more than once and is rarely physical. Alternative: keep `laplace_nd` general by making it the sixth intrinsic, which the "reads a string" rule would then no longer describe.


| Q6 (laplace_nd proper TF) | "Accept the restriction (Recommended)" | `D >= 1`, `N <= D` becomes a rule of §22.2.14 (`E0304`) |

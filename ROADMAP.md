# ROADMAP.md — Piperine

**Rewritten 2026-11-21.** Forward-looking only: what is in development, what waits and in what
order. Delivery history lives in git, `.specs/STATE.md` and each feature's `verification.md`;
open defects live in `GAPS.md`.

Rules that hold for every item: **fail loud** (what the toolchain cannot do is a named error,
never a silent no-op); **code follows the spec** (a divergence is a defect, not a documented
state); **ngspice parity first** (twins under the single `ToleranceContract`; tolerances are
never retuned).

---

## North star

Piperine is one language and one toolchain for mixed-signal design: MSL describes the circuit
— analog, digital and the boundary between them — and the same project checks, builds, runs,
tests and verifies through the `piperine` command line and the `piperine-api` Rust library, on a
native solver cross-validated against ngspice.

---

## Development pool

Each feature lives in `.specs/features/<name>/`. Up to three lanes run at a time, each with its
own worktree and target directory. Implementation on Sonnet, verification on Opus, planning on
Opus or Fable; a feature merges to `main` and upstream when its verifier passes.

### In flight

| Feature | What it delivers | Progress | Lane |
| --- | --- | --- | --- |
| `composite-values` (+ `value-semantics`, MD-76) | Structs, tuples, payload enums, arrays and ranges, `Option` of aggregates, iterators (Appendix H.5–H.7), `normal`; value semantics: no borrows, moves, `Clone` or `Copy`, every `let` reassignable (`E0903`, `E0161`) | B1–B10 done (T1 to T63, MD-79); phase C (arrays of carriers) delivered T64 to T90; then verifier, merge | C |
| `explicit-unknowns` B1–B3 | The spec and Book amendment of the explicit model (MD-78): no hidden known flag, `Logic`, `Option`, `Default`, traits as capabilities, `Resolve<X, V>` on natures, strengths, the ABI without known bytes | done on the branch; user reads the Book, then Opus verifier and merge to `main` ahead of the code | A |

### Waiting, in order (all unblock when `composite-values` merges)

| # | Feature | What is left | Waits on |
| --- | --- | --- | --- |
| 1 | `explicit-unknowns` phase A (B4–B7) | No hidden known flag: values are what their type says, `Default` decides whether a carrier may start without a value (`E0187`), `analog` reads binary carriers directly, the ABI without known bytes (still major 1), `E0511`; the spec and Book amendment (B1–B3) merges to `main` first (MD-78, user 2026-11-21) | composite-values |
| 2 | `generics` | Generic structs, enums, functions and traits, const generics, `Fn` values (phases A to C delivered: L4, L7, L8, L9, L30, L31, X6 and S3 closed; phase D delivered: L29 and L32 closed; L78 to L80, found by the phase D verification, closed by its fix round 2, which found L81; phase E delivered: L40, L58's generics group, L77, L88 and L89 closed; left: nothing; the wrap of `idtmod`, S9, closed by `fix/idtmod-wrap`); tasks amended for value semantics and explicit unknowns (`impl` on natures) | explicit-unknowns A |
| 2C | `generics` phase C (`tasks-c.md`, T53–T82 plus T116 and T117, batches B9–B13; generic modules and generation behind an interim gate until T71/T77/T80, amendment 2026-11-23) | L70, then `cast`/`same` compiled from `std/values.msl` (moved from `explicit-unknowns` T48, user 2026-11-23), then GN-32 (`encode`/`decode` of every layout, generic hand-written `Repr`, default levels: L69, L70); then generic modules (S3), placement inference, `Short`, structural params, generation `for`/`if` in a body and a `digital` block, recursion by generation, the ladder (L30, L31); flips digital#21, cheatsheet#2, cheatsheet#3, core-std#8, modules#1, #2, #6, #8 and their claims | B9: nothing (may start now); B10 on: `composite-values` phase C on `main` |
| 2D | `generics` phase D (`tasks-d.md`, T83–T102, batches B14–B16) | Templates as values, placement through a template or a binning function, `Fn(P)` bounds, trait impls on module types (the swappable MOSFET; `E0130` leaves the spec), L32; `temperature()` (core) and `multiplicity()` (a stdlib MSL function over `simparam("multiplicity")`, with `DEFAULT_MULTIPLICITY = 1.0`; no core row, no `--multiplicity` key, no `E0802`, no IR/ABI read — user decision 2026-11-23: "É, melhor voltar Option") as `Option<f64>` run settings the model reads, no implicit params (user decision 2026-11-23, L29); T99–T101 need `analog-operators`' `simparam -> Option<f64>` on `main`; flips modules#12 and modules~1faf8c12; `resolved-nets-bundles` owns modules#10 and cheatsheet#8; one multiplicity per run, an instance combines its own `m` param as `m.or(multiplicity()).unwrap_or(DEFAULT_MULTIPLICITY)` (user 2026-11-23) | generics C; delivered: templates and `Fn(P)` bounds (B14 and B15, T83–T93, L32 closed) and the run settings `temperature()` and `multiplicity()` (B16 (T94–T102), L29 closed); the verification's fix round 1 added `inst.name` in the placing module, templates of generic modules and a module `impl` through the bound, and left L78 to L80 to `spec-closure`; fix round 2 closed L80 (a param default reads the params before it, §8.1.16) and L79 (the type `Name<G>` of a template carries its arguments, §8.4.3) and L78 (an item of a child placed through a template read as a value, §8.4.8), and found L81 (a `let` over the solve or a carrier of a child's child) for `spec-closure`. |
| 2E | `generics` phase E (`tasks-e.md`, T103–T120, batches B17–B18; delivered) | The L58 generics group against §22 (`merge` struck, MD-78), `std` under the whole-model check; closes the feature (`checks.md`, Verifier) | generics D, `analog-operators` B4–B7 (user 2026-11-23) |
| 3 | `explicit-unknowns` phase B (B8) | Operator traits, `Logic<N>` from source in `std/logic.msl` (L68: `Logic`, `logic`, `matches_bits`) | generics A |
| 4 | `analog-operators` | Delivered, B1–B7 (MD-82): `ddx`, `delay`, `simparam(name) -> Option<f64>` (no run `tnom`: a per-instance model param, user 2026-11-23), `noise`, `white_noise` and `flicker_noise` with the noise analysis (per-source totals, `onoise`/`inoise`, the `noise` run and `--analysis noise`), `emit!` in `analog` as traces, and the retaken step that makes `slew` limit an ideal step (L43–L45, L47, L49, L60, U7 closed; L72 found and closed); the ngspice twins `delay_tline`, `noise_rc`, `noise_diode`, `tnom_res` agree, `noise_switch` is held to its closed form (ngspice reads a closed switch as open in `.noise`) | composite-values |
| 5 | `unknown-semantics-events` | Delivered, B1–B5 (MD-74): `settle`, held settle commits, events read as `bool` (`Pfd3`), `limexp`, the Book's clipper and buck, functions that return `Event` (L51, L52, L53 closed) | generics A–B |
| 6 | `params-and-io` | Delivered, B1–B6 (MD-75): the host's node sources, integer, enum (payload enums included) and `Option` params, `E0122`, `parse_table`, `parse_hex`, `table_model`; B5–B6 ran on composite-values and explicit-unknowns A | composite-values, explicit-unknowns A |
| 7 | `resolved-nets` | **Delivered** (MD-83): `Resolve<X, V>` as a capability of a nature, `Strength` (five levels), `Driven`, `strongest`, `Pullup`, `Pulldown`, `Keeper`, `Tribuf`, `Tran`, `Tgate`, `Bidir`; `E0220`, `E0225`, `E0511`; a parent drives a child's `pub` resolved signal; the Book's net rows run (L21, L22, L23 and the net group of L58 closed; L71 the host's `Logic` pattern) | generics, explicit-unknowns B |
| 8 | `resolved-nets-bundles` | B2 and later: connecting and resolving bundles (L25) | resolved-nets |
| 9 | `code-clarity` | Readability and structure refactor under the seven cleanliness rules, CC-11 included (no language special cases: builtins declared in the MSL prelude), and the split of the three oversized `pom/{resolve,reflect,lower}/mod.rs`; a Fable inventory first | items 1–8 |
| 10 | `composite-values` phase D | Number widths (L13, CV-29 to CV-33): `f32` as Rust's, `f16` and `f128` correctly rounded in every operation and H.2 method (user Q4 (a), 2026-11-23), integers of any width; the leaf crate `piperine-numbers` embedded as runtime source. Delivered (2026-11-23): `design-d.md`, `tasks-d.md` (T91 to T126, batches D1 to D6); a `Logic<N>` of more than 64 levels runs | composite-values phase C (D4 to D6 only) |
| 11 | `spec-closure` (**item 13**) | Fable closure of spec ↔ implementation; G1, G2, G4, G5, X2, H13 (X1 closed by `fix/idtmod-wrap`) and the discrepancies logged during the run | code-clarity, composite-values phase D, and **no Book listing still refused by a feature** (below) |

`unknown-semantics-per-level` is **dropped**: explicit unknowns replace per-level flags (user, 2026-11-21).

**Every Book listing runs before item 13** (user, 2026-11-21: "as listagens que não rodam precisam rodar [...] Tem que colocar em alguma feature ANTES do spec closure"). A listing that cannot run yet is a `refused` row of `tests/book/inventory.tsv` (or an `unraised` claim) with a `GAPS.md` §11 row naming the feature that makes it run, and its test asserts today's refusal, so it fails "now runs" the day it runs. Each owning feature's closing task flips its rows; a feature does not close with one of its listings still refused. `spec-closure` does not start while any row is owned by a feature of this table. The owners of the rows the explicit-unknowns amendment left refused:

| Listings | Refused today with | Made to run by |
| --- | --- | --- |
| welcome#17, analog#9, modules#9, mixed#8, mixed#10, mixed#14, values#21 (`analog` reads a discrete carrier directly) | `E0150` | `explicit-unknowns` phase A (T20, closed by T40) |
| The `E0187` and `Default` initial-value sentences of the values chapter | not raised | `explicit-unknowns` phase A (T21, T22, T30) |
| values#14 (`Option` as a carried type) | `E0112` | `explicit-unknowns` phase A |
| values#18 and the other `Logic<4>` listings | `P0001` | `explicit-unknowns` phase B (T48) |

**The automatic run stops after item 13** for the user's review of the closure, the spec and the
code (user, 2026-11-18 and 2026-11-21).

### After the review

| Feature | What it delivers | State |
| --- | --- | --- |
| `transient-performance` | Faster transients: only observable landings, the tested landing and jump steps, stamps reused, no instant at an idle step, T25's tick epoch. The buck's start went from 804 136 steps / 151.9 s to 210 188 / ~27 s | **delivered** — verified (round 4) and merged (user, 2026-11-22) |
| `digital-performance` | A digital-only run in bounded memory, no quadratic paths, an allocation-free call, work proportional to activity; last, the whole digital circuit of a compiled unit evaluated in topological order over a flat state until it converges (Verilator-style). Measured today: 175x (counter) to 560 000x (10 000 flops) behind Verilator; memory grows with run length | spec on `feature/digital-performance` (diagnosis + bench harness); **postponed until after spec-closure and the code-clarity refactor** (user, 2026-11-22: "precisamos primeiro de um bom refactor e de todas as features funcionando") |
| `host-iii` | `force`, `release`, waits for an event, continued transients (H1, H6) | spec |
| `host-analyses` | S-parameters, Fourier, distortion, operating-point variables (H2) | spec |
| `live-param-defaults` | A param default recomputed on a host `set` of the param it reads (via `Params`/`Derive`, MD-69), so that param stays live — relaxes `E0803`; option 2 of the 2026-11-24 structural-param decision | spec (user, 2026-11-24) |
| `sweeps-montecarlo-optimization` | Multi-axis sweeps, Monte Carlo, optimization (H2, H7) | spec |
| `gradients` | Adjoint gradients (H8, `E0807`–`E0809`) | spec |
| `cli-packages` | Packages from the command line (H10) | spec |
| `conformance` | The galleries and conformance on the compiled backend (U1, U2, U4, U6, U8, U9) | spec |
| `foreign-models` | Foreign and OSDI models, linking (L12, L33); writes its own Book chapter and listings | spec |
| `multi-rate` | Digital edges that do not cut analog steps (V12) | delivered by `transient-performance` (landings only where something observable happens: `59be2e5c`; no instant at an idle step: `c8569a22`) |
| `surrogate-models` | Averaged or surrogate models for long runs (V13) | spec; later (user) |
| `periodic-small-signal` | Small-signal analyses over the periodic steady state (V16) | spec |
| `ngspice-models` | The ngspice device models in the MSL standard library — same names, params and equations, each with a parity twin — so a SPICE user moves over easily | **idea only**; well after the rest (user, 2026-11-21) |
| `device-plugins` | A plugin scheme for devices not written in MSL: a separately built library implementing the device ABI, in its own repository and package (licence boundary: a GPL plugin is fine beside MIT Piperine), placed from MSL through `extern "cosim"` / `#[link]` and loaded via Session/CLI | idea in `.specs/features/device-plugins/idea.md`; **much later** (user, 2026-11-24) |
| `arduino-simulator` | An Arduino / AVR firmware co-simulated with the circuit: our own Rust AVR core (based on avr8js, MIT; per-core facts distilled from datasheets and simavr), pins through the device ABI, ADC/UART/interrupts | discussion + proof of concept in `.specs/features/arduino-simulator/discussion.md`; **depends on `device-plugins`**; **much later** (user, 2026-11-24: "coloca lá para depois, bem depois") |
| `esp32-simulator` | An ESP32 firmware co-simulated with the circuit through Renode (MIT, to confirm; QEMU fork as the alternative) behind a device plugin: GPIO, LEDC, ADC, DAC, UART; pull-on-read sync with validity windows | idea in `.specs/features/esp32-simulator/idea.md`; **depends on `device-plugins`**; **much later** (user, 2026-11-24) |
| `python-host` | A Python host | deferred until the host features are done (user, 2026-11-18) |
| `lang-server` | The language server, rebuilt on the POM's reflection (the old crate and the VS Code extension were removed by `code-clarity`) | last (user) |

---

## Decisions waiting for the user

Logged during the unattended run; presented at the item 13 review.

| Topic | Question |
| --- | --- |
| Resolved nets | The `driving` field in the device ABI |
| Unknowns per level | The per-level known-byte asserts in the buck; the `f64` mask |
| Spec §13 | D2 |
| Book | The clipper twin |
| Buck example | Brown-out OVP, start-up peak iL 1.78 A, the 12 → 9 V ramp of 236 µs |
| Elaboration | A child's node connected to another placement (`P0001`) is a lowering gap, not a spec rule: close it, or spec it |

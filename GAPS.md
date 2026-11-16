# GAPS.md — what does not work yet

**Rewritten 2026-11-15**, after M1 (the first MSL → solver path) merged; **updated 2026-11-16**
after `msl-digital` (digital and mixed MSL from source). The previous version
(2026-08) audited the retired PHDL / Verilog-AMS / Cranelift stack; none of that code exists any
more, so it was dropped rather than amended.

This file lists **known gaps**: things that are missing, wrong, or unproven today. Planned work
and its order live in `ROADMAP.md` ("Queue after ir-rust"); this file says what a user hits
*now*. Remove an entry in the commit that closes it.

Ranking, most serious first: **silent** (accepted, then wrong or ignored) › **broken** (does not
build or run) › **loud** (refused with a diagnostic) › **spec hole** (the spec does not say) ›
**unproven** (works, nothing falsifies it).

---

## 1. What runs today

| Path | State |
| ---- | ----- |
| `.msl` → parse → POM → resolve | every stdlib module, gallery model and `examples/buck` |
| type check (`msl-typeck`, `msl-digital`) | the M1 analog subset and the digital and mixed forms `examples/buck` uses (integers, unit enums, `match`, `Option`, carriers, definitions, events, reactions, commits, records, modules with a result, `now()`, `analysis(Static)`, a flow probe under a potential contribution); anything else is `P0001` by name (§4) |
| elaboration → IR → interpreter → solver | analog only: R, C, V/I sources, diode with `limit`/`pnjlim`; OP, DC, AC, transient. The interpreter is frozen at M1 (R1) and refuses every digital or amended form by name |
| ngspice parity | 7 fixtures in `tests/ngspice/`, all green |
| IR → Rust → `rustc` → ABI (`ir-rust`) | the M1 analog subset compiles to Rust, bit-equal to the interpreter, 7 twins green on both backends (`PIPERINE_BACKEND`) |
| digital and mixed-signal execution (`msl-digital`) | **from MSL source, on the transpiled backend only**: the whole `examples/buck` elaborates, compiles with no `rustc` warning and runs; its closed loop starts and regulates at 0.8 V (`cargo test -p piperine-parity --test buck_closed_loop`). The stdlib items the buck reaches run too: `transition`, `A2D`, `D2A`, `above`, `init`, `every`, `Never`, `log`, `require`. Only from a Rust test (`piperine-parity::Project`): no host drives it yet (§2) |
| CLI / host API / LSP | **do not build** (§2); the language server is deferred |

---

## 2. Broken — the user-facing surface does not build

- **`piperine-api`** (165 errors), **`piperine-cli`** (43), **`piperine-lang-server`** (91), and the
  root package `piperine` (it re-exports `piperine-api`) fail `cargo check`. They still target the
  deleted VAMS front end and the deleted `Element` solver interface. All four sit outside
  `default-members`, so `cargo build` stays green while they rot. Consequence: **an MSL project
  can only be run from a Rust test** (`piperine-parity::{Bench, Project}`). The CLI and the
  project are queue item 3 of `ROADMAP.md`; **the language server is deferred** to a feature of
  its own after them (user: "lang-server é bom ficar... cli também mas ambos vão ter sua própria
  feature"), so it stays unbuilt through the CLI work.
- **`piperine-project`** builds, but only as manifest + dependencies + lockfile; nothing on the
  run path consumes it.

---

## 3. Silent

- **A derived child param is not re-derived on a host `set` (`msl-digital`).** A placement
  that gives a child param a value reading the parent's params (`PowerStage { ron: ron }`) is
  evaluated once per instance at elaboration, and the instance table holds the constant. A host
  `set` of the parent's param during a run changes the parent only: the child keeps the value
  derived at elaboration, with no diagnostic. No host can `set` yet (§2), so nothing hits it
  today; the host surface owes the re-derivation or a refusal.
- **A child's own signals read as `gnd` in the authored `Child` (msl-elab).** `Child::nodes` pads
  each `signal` of the placed module with `Ground`, because `Terminal` cannot name the child's own
  node. The flat instance table carries the right node, so simulation is correct, but anyone
  walking the IR's child records sees ground. Needs `Terminal::Own` in `piperine-ir`. No M1
  device has a signal, so no test shows it.

---

## 4. Loud — refused, though the spec allows it

- **An MSL local named like an MSL function of the same body** (a param `vt` in a body that calls `vt()`) makes the generated Rust fail to compile (`ir-rust`). Generated names can no longer collide with MSL names (F2/F3), but MSL-vs-MSL shadowing across the value/function namespaces is not renamed yet. A `let`-bound closure named like a function could in principle be called instead of it.

- **An unknown control is a fault (`msl-digital`, `ir-rust` RS-11 AC 5).** An `if`, a `match` or
  a reaction guard whose condition is unknown at run time stops the run with a fault naming its
  origin. §3.3.3 instead merges the effects of both branches; that merge is M2. Every decision of
  the buck is written with a `known_or`, so it never faults.
- **Enum variant names are not carried (`msl-digital`).** A `Bit<Mode>` carrier is its tag,
  `UInt<ceil(log2 V)>`, in the IR, on the ABI and in every observation: a host reads `2`, not
  `Regulate`. The names belong to the host surface.
- **Everything outside the subset of M1 and `msl-digital`** is `P0001` naming the construct, at
  type check or elaboration (DIG-07 AC 1): loops, `let mut`, assignments, structs, arrays and
  indexing, closures other than a `limit` function, user traits and `impl`s, generic modules and
  functions, enums with a payload or an explicit discriminant, natures with `resolve` (`Wire`,
  `Tri`), bundles, `others()`, `settle`, element and field commits, `delay`, `noise`, `ddx`,
  `bound_step`, `freq()`, `simparam`, `analysis` of a kind other than `Static`, generation `for`
  and `if`, templates, `extern` modules, a `ttol` or `etol` given to `cross`, a binding inside
  `Some` in a definition, an `||` head split into reactions whose body holds `emit!`, and an
  event read as a `bool`. The stdlib items built on them (`Bidir`, `slew`, `idt`, `idtmod`,
  `zi_*`, `laplace_*`, `inertial`, `absdelta`, `last_crossing`) are refused with them.
- **The interpreter is frozen at the M1 analog subset** (user decision R1, 2026-11-15). Digital
  and mixed forms run on the transpiled path only; the interpreter refuses each by name.
- **`ir-rust` refuses** `UInt<N>`/`Int<N>` with N outside 1..=64 by name (`check()` and the
  printer); the ABI's leaves end at 64 bits. **`Float<16>` and `Float<128>`** have no IR form (the
  IR's one real is `f64`): the type checker refuses them by name (`P0001`, `msl-digital`).
- **A `limit` or `ac_stim` function reading a carrier is refused by name (`ir-rust` F-B6).** No
  lowering decodes a carrier in those functions yet; no body of the buck slice has one.

---

## 5. Spec holes — the spec does not decide

Each has a provisional rule so work is not blocked; the spec sentence is still owed. Details in
`examples/buck/GAPS.md`.

- **G4 — `f64 as u10` outside the range.** Part I §1 rule 1 says "truncates or extends, as in
  Rust", which is ambiguous for widths Rust lacks. Provisional: saturate, NaN → 0.
- **G5 — shift by N or more bits.** No rule. Provisional: the mathematical result truncated to
  N bits (0 for `<<` and for unsigned `>>`, sign fill for signed `>>`). A negative amount has no rule
  either; compiled code panics naming the shift (a fault, `ir-rust` F-B9).
- **G6 — unknown integers are per value, not per bit (`ir-rust` F-B4).** §3.3.2 gives `&`, `|`,
  `^`, `!` per level, so a partial mask leaves known bits: `(x & 1) & 2` with `x` unknown is known
  0. The ABI has one known byte per value, so the compiled code applies §3.3.2's shortcuts only
  when one operand is all-zeros (`&`) or all-ones (`|`); otherwise the result is unknown, and a
  later `known_or(d)` returns `d` where §3.3.2 gives the known bits. Per-bit flags would change the
  ABI; ruling 2026-11-15, the user may revisit.
- **G1 — no fixed-point type; G2 — no ranged integer.** Language proposals the buck wanted, not
  bugs.

---

## 6. Solver — open points

From the solver internals review (artifact `TEV6Hqo6d3QRi1ssZtJTW5`, "Pontos para decidir") and
the solver-abi-integration verification. None breaks the 7 parity fixtures today; the first two
are the parity risks.

- **The step is controlled by LTE on node voltages, not on charge.** ngspice controls it on
  charge; `chgtol` is unused. Expect divergence on circuits with large or nonlinear charges.
- **The Milne predictor's skip is asymmetric** compared with ngspice.
- **A step that reaches `dt_min` is accepted** instead of failing the analysis.
- **Newton damping is one global L2 factor**, not per unknown.
- **Dead hooks:** `apply_limit` / `apply_limiting_reports` are never called.
- **Stale doc:** `sp.rs` still documents the PHDL `@rfport`.
- **The digital checkpoint clones every net** on each mark.
- **Re-entry is approximate (N1):** re-entering a transient at a captured step ends up to
  7.3e-4 relative off the continuous run.
- **Re-entry state is stored on every accepted step (N2):** memory grows with step count.
- **The partials guard matches devices by label (N3)**, not by module.
- **A transient keeps every accepted step, and a mixed design takes many (`msl-digital`).** The
  whole buck takes about 45 000 accepted steps per 40 µs (the analog side lands near every
  200 MHz instant), each about 58 KB in `TransientAnalysisResult`: a 600 µs run held 25 GB
  before it was stopped. `buck_closed_loop` keeps only its last 40 µs (`record_from`); a host
  needs a bounded recording (selected probes, a decimated grid) before long mixed runs. The same
  run takes 779 s in a dev build.
- **First step after a digital-to-analog jump (`ir-rust` F-B7).** With a 10 ns step cap the
  analog response to a carrier change is off by more than `TRAN_VOLTAGE` 5.55 ns after `t0` (the
  first step after the jump is first order); `piperine-parity/tests/boundary.rs` runs at a 1 ns
  cap, 20x tighter than `parity_tran`'s, to pass. The restart step after a boundary event needs
  the solver's attention.

---

## 7. Unproven

- **Parity covers 7 small analog circuits.** No MOS, BJT, JFET, nonlinear capacitance, noise, or
  switching transient. Two fixtures (`diode_iv`, `tran_rc`) sit near the tolerance floor, so a
  small regression there may still pass.
- **No oracle for digital or mixed-signal behavior.** The interpreter cannot run them and no
  ngspice twin exists for MSL digital; `ir-rust` phase B and `msl-digital` test them against the
  spec's own expected values only (`docs/spec/`, `examples/buck/SPEC.md`).
- **The buck's closed loop is proven at one operating point**: setpoint 0.8 V, Vin 12 V, 6.6 Ω,
  600 µs. The bench scenarios S1 to S9 of `examples/buck/SPEC.md` §10 are queue item 4.
- **Parity tests are scattered** (`tests/ngspice/` + `piperine-parity`); the planned single root
  `tests/e2e/` home does not exist yet.

# GAPS.md — what does not work yet

**Rewritten 2026-11-15**, after M1 (the first MSL → solver path) merged. The previous version
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
| type check (`msl-typeck`) | the M1 subset; anything else is `P0001` |
| elaboration → IR → interpreter → solver | analog only: R, C, V/I sources, diode with `limit`/`pnjlim`; OP, DC, AC, transient |
| ngspice parity | 7 fixtures in `tests/ngspice/`, all green |
| IR → Rust → `rustc` → ABI (`ir-rust`) | in progress on `feature/ir-rust` |
| digital and mixed-signal execution | **none**: they parse and resolve, nothing runs them |
| CLI / host API / LSP | **do not build** (§2) |

---

## 2. Broken — the user-facing surface does not build

- **`piperine-api`** (165 errors), **`piperine-cli`** (43), **`piperine-lang-server`** (91), and the
  root package `piperine` (it re-exports `piperine-api`) fail `cargo check`. They still target the
  deleted VAMS front end and the deleted `Element` solver interface. All four sit outside
  `default-members`, so `cargo build` stays green while they rot. Consequence: **an MSL project
  can only be run from a Rust test** (`piperine-parity::Bench`). Queued right after `ir-rust`.
- **`piperine-project`** builds, but only as manifest + dependencies + lockfile; nothing on the
  run path consumes it.

---

## 3. Silent

- **G3 — an Appendix H method on a carrier receiver stays `Unresolved` with no diagnostic.**
  `oc_count.saturating_add(1)` on a `Bit<u4>` inside a reaction resolves to nothing, and the
  front end says nothing (`pom/resolve/members.rs`). `examples/buck` works around it, and its test
  asserts no member stays unresolved, so the buck itself cannot regress silently.
- **A child's own signals read as `gnd` in the authored `Child` (msl-elab).** `Child::nodes` pads
  each `signal` of the placed module with `Ground`, because `Terminal` cannot name the child's own
  node. The flat instance table carries the right node, so simulation is correct, but anyone
  walking the IR's child records sees ground. Needs `Terminal::Own` in `piperine-ir`. No M1
  device has a signal, so no test shows it.

---

## 4. Loud — refused, though the spec allows it

- **Everything outside the M1 subset** is `P0001` at type check or elaboration: digital
  definitions, reactions, commits, events, `match`, loops, `let mut`, structs, enums, `idt`,
  `delay`, `noise`, `transition` and the rest of the stdlib's analog operators.
- **The interpreter is frozen at the M1 analog subset** (user decision R1, 2026-11-15). Digital
  and mixed forms will run on the transpiled path only.
- **`ir-rust` will refuse** `Float<16>`, `Float<128>` and `UInt<N>`/`Int<N>` with N > 64, by name;
  stable Rust has no type for them.

---

## 5. Spec holes — the spec does not decide

Each has a provisional rule so work is not blocked; the spec sentence is still owed. Details in
`examples/buck/GAPS.md`.

- **G4 — `f64 as u10` outside the range.** Part I §1 rule 1 says "truncates or extends, as in
  Rust", which is ambiguous for widths Rust lacks. Provisional: saturate, NaN → 0.
- **G5 — shift by N or more bits.** No rule. Provisional: the mathematical result truncated to
  N bits (0 for `<<` and for unsigned `>>`, sign fill for signed `>>`).
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

---

## 7. Unproven

- **Parity covers 7 small analog circuits.** No MOS, BJT, JFET, nonlinear capacitance, noise, or
  switching transient. Two fixtures (`diode_iv`, `tran_rc`) sit near the tolerance floor, so a
  small regression there may still pass.
- **No oracle for digital or mixed-signal behavior.** The interpreter cannot run them and no
  ngspice twin exists for MSL digital; `ir-rust` phase B tests them against the spec's own
  expected values only.
- **Parity tests are scattered** (`tests/ngspice/` + `piperine-parity`); the planned single root
  `tests/e2e/` home does not exist yet.

# Piperine — Gap Analysis & Development Guide

> **Purpose.** This document is the authoritative, non-ambiguous guide to every
> divergence between the language design (`docs/piperine-hdl-spec.md`) and the
> current implementation. It is written so that an AI agent (or human) with no
> prior context can pick a section, understand the problem, understand the
> rationale, implement the fix, and verify it.
>
> **How to use it.** Each gap is self-contained and follows a fixed template:
> Spec reference → Current state (with `file:line` citations) → Why it matters →
> Proposed solution (with code sketches) → Decision rationale → Verification
> (tests to write) → Acceptance criteria. Work top-to-bottom within a Part;
> Parts are ordered by dependency (later Parts assume earlier ones are done).
>
> **Conventions.** All `file:line` citations are 1-indexed and relative to the
> repo root. Code sketches are Rust unless marked `phdl:` / `va:`. "Spec §X.Y"
> always refers to `docs/piperine-hdl-spec.md`. "IR-SYSTEM §Z" refers to
> `crates/piperine-codegen/IR-SYSTEM.md`. Before starting any task, read
> `AGENTS.md` in full — it has the build/test commands and the frozen-file rules.

---

## Table of contents

- [Part 0 — Prerequisites & reading order](#part-0--prerequisites--reading-order)
- [Part A — Silent wrong-code bugs (fix first)](#part-a--silent-wrong-code-bugs-fix-first)
- [Part B — Type system & the no-magic rule](#part-b--type-system--the-no-magic-rule)
- [Part C — Standard library & prelude](#part-c--standard-library--prelude)
- [Part D — Codegen: forces, analog operators, noise, functions](#part-d--codegen-forces-analog-operators-noise-functions)
- [Part E — Mixed-signal bridges (A2D / D2A)](#part-e--mixed-signal-bridges-a2d--d2a)
- [Part F — `from_ir`: hierarchy, recursion, parent bodies](#part-f--from_ir-hierarchy-recursion-parent-bodies)
- [Part G — AMS frontend gaps](#part-g--ams-frontend-gaps)
- [Part H — Solver: integration, timestep, convergence](#part-h--solver-integration-timestep-convergence)
- [Part I — PHDL language features (generics, capabilities, bundles, enums, higher-order)](#part-i--phdl-language-features-generics-capabilities-bundles-enums-higher-order)
- [Part J — Diagnostics, events, `$assert`](#part-j--diagnostics-events-assert)
- [Part K — Architecture cleanup](#part-k--architecture-cleanup)
- [Part L — Documentation & visibility](#part-l--documentation--visibility)
- [Appendix — Build, test, and frozen-file rules](#appendix--build-test-and-frozen-file-rules)

---

## Part 0 — Prerequisites & reading order

### 0.1 Architecture at a glance

```
   .va / .vams    ──►  piperine-ams    ──►      ┌──────────────────┐
   (Verilog-A/AMS)  ◄─►   frontend     ◄─►      │  piperine-codegen │
                                                │   (IR + lowering)  │
   .phdl / .ppr    ──►  piperine-lang  ──►      └────────┬─────────┘
   (PHDL / .ppr)     ◄─►   frontend     ◄─►               │
                                                            ▼
                                                  Vec<Box<dyn Device>>
                                                            │
                                                  ┌─────────┴─────────┐
                                                  ▼                   ▼
                                       ┌──────────────────┐  ┌──────────────────────┐
                                       │  piperine-solver   │  │  piperine-solver OSDI │
                                       │  (Newton-Raphson,  │  │  (.osdi shared libs)  │
                                       │   trapezoidal,     │  │                         │
                                       │   mixed-signal)     │  └──────────────────────┘
                                       └──────────────────┘
```

The **IR (`crates/piperine-codegen/src/ir.rs`) is the only contract** between
frontends and the solver. `piperine-solver` does **not** depend on
`piperine-codegen`; the codegen depends on the solver (`Device`,
`CircuitInstance`) because it lowers IR into it. **Breaking this arrow is a
regression** — verify with `cargo metadata` if in doubt.

**NGSPICE faithful headers.** The 8 model files in
`crates/piperine-lang/headers/ngspice/` (`res`, `cap`, `ind`, `mut`, `dio`,
`bjt`, `jfet`, `mos1`, plus `sw`, `csw`, `vsrc`, `isrc`, `vcvs`, `vccs`,
`ccvs`, `cccs`) are the gold standard for "what should work." They are
100% faithful to NGSPICE's C source and exercise every gap listed in
this document. When a gap is closed, the corresponding NGSPICE model that
previously failed at parse / elaborate / codegen should now pass.
Detailed gap inventory and cross-reference: `docs/NGSPICE_GAPS.md`.

### 0.2 Crate map (current, real)

| Crate | Role | Key files |
|-------|------|-----------|
| `piperine-ams` | Verilog-A/AMS frontend | `src/{lexer,parser,preprocessor,grammar,ast,model,fmt}.rs`, `headers/*.vams` |
| `piperine-lang` | PHDL frontend (parse + elab) | `src/parse/`, `src/elab/`, `src/resolve/`, `src/stdlib/` |
| `piperine-codegen` | IR + lowering to `Device` | `src/ir.rs`, `src/from_ams.rs`, `src/from_ppr.rs`, `src/from_ir.rs`, `src/ir_analog_to_device.rs`, `src/ir_digital_to_interp.rs`, `src/phdl_device.rs`, `src/codegen/` |
| `piperine-solver` | Newton-Raphson, AC/DC/Tran/Noise/TF, OSDI | `src/{analog,digital,osdi,solver,math,topology}.rs` |
| `piperine-cli` | clap subcommands | `src/commands/{check,fmt,build,run,test,new,clean}.rs` |
| `piperine-project` | `Piperine.toml` reader | `src/lib.rs` (63 lines) |

### 0.3 Build & test (mandatory before declaring done)

```sh
cargo build                  # build the workspace
cargo test                   # ~260 tests; must pass (see tests-baseline.md)
cargo test -p piperine-codegen   # always re-run after touching codegen
```

For OSDI tests: `OPENVAF_BIN` must be in PATH (auto-downloaded by
`piperine-solver/build.rs` on linux x86_64).

### 0.4 Frozen files (DO NOT EDIT)

Per `AGENTS.md`: everything under `crates/piperine-ams/tests/fixtures/`,
`crates/piperine-ams/tests/fixtures_fmt/`, `crates/piperine-ams/tests/fixtures_ppr/`,
`crates/piperine-ams/headers/`, and `crates/piperine-solver/tests/va/` is a
frozen test corpus. Do not modify these files. New test fixtures go in new
files under the appropriate `tests/` directory.

### 0.5 Conventions to follow in new code

- **Panics:** never `unwrap()`/`expect()` on user-input paths; return
  `Result<_, E>`. `unwrap()` is acceptable only behind a provable invariant
  (e.g. `peek`-guarded lexer reads), and should carry a `// SAFETY:` comment.
- **Fail-loud over silent zero:** the codegen uses
  `CodegenError::Unsupported(...)` rather than `todo!()`/`unimplemented!()`.
  Keep this discipline. **Never** add a new `_ => IrExpr::Real(0.0)` or
  `_ => f64const(0.0)` fallback arm without a `validate_*` guard upstream.
- **Comments:** do not add comments unless asked; keep module-level `//!`
  docblocks updated when adding a new entry point.
- **Numeric conventions:** analog = `f64`; digital = `LogicValue`; mixed-signal
  nets = anonymous `usize` indices.
- **No new dependencies** without checking `Cargo.toml` and the workspace
  `[workspace.dependencies]` table first.

### 0.6 Severity legend

- **Critical** — breaks a core promise of the spec; silent wrong results.
- **High** — blocks a whole class of spec examples from running.
- **Medium** — feature is parse-only or partial; not silent-wrong.
- **Low** — polish, docs, ergonomics.

---

## Part A — Silent wrong-code bugs (fix first)

> These are the highest-priority items. They compile, run, and produce
> *plausible but incorrect* results. They violate the fail-loud discipline
> more than any other class of work, and they erode trust in everything that
> *appears* to work. Fix them before adding any new feature.

### A.1 `BranchAccess "I"` reads inside an analog contribution silently emit 0

**Spec:** §8.1 — `I(a, b)` is a documented branch access function. A model may
read flow in a contribution expression (e.g. a controlled source that depends
on a sensed current).

**Status:** WRONG-CODE / silent zero. **Critical.**

**Current state.** `validate_ir_contrib` accepts `access == "I"`
(`crates/piperine-codegen/src/codegen/ir_emit.rs:429`), so the construct passes
validation. But `emit_ir_expr` then emits `f64const(0.0)` for any `I(...)` read
(`ir_emit.rs:102-106`):

```rust
IrExpr::BranchAccess { access, plus, minus } => {
    if access == "V" {
        // ... lookup branch_voltages, fall back to 0.0 if unknown ...
    } else {
        // I(a,b) and other flows are not available in the KCL stamp context;
        // their reactive/source handling lives elsewhere.
        f64const(0.0)   // <-- SILENT WRONG VALUE
    }
}
```

A model like `I(p, n) <+ V(p, n) / r + I(sense, gnd) * gain;` compiles, runs,
and silently drops the `I(sense, gnd) * gain` term.

**Why it matters.** This is the textbook VCCS/CCCS pattern. The spec's
`IndirectContrib` (`I(cp,cm) : V(pp,pm) = expr`, IR-SYSTEM §5) is the explicit
form, but a plain `I(...)` read in a contribution is also legal and common.
Silent zero here makes every current-sensing model wrong without diagnostic.

**Proposed solution.** Two options, in order of ambition:

**Option 1 (minimal, fail-loud):** Make `validate_ir_contrib` reject `I` reads
in contribution expressions so the user gets a clear error instead of wrong
results. This matches the current state (no flow-read support) without lying.

```rust
// ir_emit.rs:428-434 — change to:
IrExpr::BranchAccess { access, .. } => {
    if access == "V" {
        Ok(())
    } else {
        Err(unsupported(format!(
            "reading flow `{access}(...)` inside a contribution is not yet supported; \
             use an indirect contribution `I(cp,cm) : V(pp,pm) = expr` instead"
        )))
    }
}
```

**Option 2 (full):** Implement flow reads by allocating a branch-current
unknown. This requires the solver to support voltage-source branch-current rows
(see H.4). The flow `I(a,b)` becomes a read of that branch-current unknown,
indexed in the MNA matrix. This is a larger solver change and is deferred to
Part H.

**Decision rationale.** Adopt **Option 1 now** (fail-loud), track Option 2 as
a follow-up under Part H. Rationale: a clear error is strictly better than a
wrong number; the full MNA branch-current extension is a solver-scale change
that should not block the bug fix.

**Verification.** Add a test in
`crates/piperine-codegen/tests/wave1_nonlinear_tests.rs` (negative-assertion
pattern, mirroring `power_law_contribution_uses_pow_not_add`):

```rust
#[test]
fn current_read_in_contribution_is_rejected_not_silently_zero() {
    // I(p,n) <+ V(p,n)/r + I(sense,gnd)*gain  -- the I(sense,gnd) read must fail
    let ir = build_ir_with_current_read();
    let err = ir_analog_to_device(&ir, "vccs").unwrap_err();
    assert!(
        err.to_string().contains("flow") || err.to_string().contains("I("),
        "expected flow-read rejection, got: {err}"
    );
}
```

**Acceptance criteria.**
- [ ] `validate_ir_contrib` rejects `BranchAccess` with `access != "V"`.
- [ ] No `_ => f64const(0.0)` arm for `BranchAccess` remains reachable for `I`.
- [ ] Negative-assertion test passes.
- [ ] `cargo test -p piperine-codegen` green.

---

### A.2 `SimQuery::Temperature` and `SimQuery::Abstime` silently emit 0

**Spec:** §8.1 — `$temperature` and `$abstime` are simulator queries a model
may read. The spec's `Diode` uses `temp` as a parameter (workable), but
`$temperature` is the canonical way to read the simulation temperature.

**Status:** WRONG-CODE / silent zero. **Critical.**

**Current state.** `validate_ir_contrib` accepts `Temperature` and `Abstime`
(`ir_emit.rs:437`), but `emit_ir_expr` emits `f64const(0.0)` for any `Sim`
variant other than `Vt` (`ir_emit.rs:143`):

```rust
IrExpr::Sim(sq) => match sq {
    SimQuery::Vt(_) => f64const(0.025852),   // see A.3
    _ => f64const(0.0),                       // <-- Temperature, Abstime: SILENT 0
},
```

**Why it matters.** A temperature-dependent model written with
`$temperature` compiles, runs, and produces results as if `T = 0 K`. This is
physically nonsensical and the user has no way to tell.

**Proposed solution.** Thread a `SimCtx` (or extend the existing param slice)
into the JIT-compiled functions so that `Temperature`, `Abstime`, `Mfactor`,
and `Simparam` are readable at runtime. Concretely:

1. The `extern "C"` signature today is
   `fn(*const f64, *const f64, *mut f64)` — `(branch_voltages, params, out)`.
   Add a fourth argument `sim: *const SimCtx` where:
   ```rust
   #[repr(C)]
   pub struct SimCtx {
       pub temperature: f64,   // Kelvin
       pub abstime: f64,       // seconds
       pub mfactor: f64,
       pub gmin: f64,
   }
   ```
2. `JitAnalogDevice` (`codegen/mod.rs:55-96`) holds a `SimCtx` field, updated
   by the solver at each `load_dc`/`load_transient` call.
3. `emit_ir_expr` for `Sim(Temperature)` → load `(*sim).temperature`; for
   `Sim(Abstime)` → load `(*sim).abstime`; for `Sim(Mfactor)` →
   `(*sim).mfactor`; for `Sim(Simparam{key,default})` → look up `key` in a
   simparam map (start by returning `default`).
4. `Sim(Vt(t_opt))` → compute `kT/q` from `(*sim).temperature` (and the
   optional `t` argument if present, else use `sim.temperature`). This also
   fixes A.3.

**Decision rationale.** A `SimCtx` struct is the minimal extension that
unlocks all simulator queries at once. It matches how SPICE-style kernels
thread sim state. The `#[repr(C)]` keeps the Cranelift ABI stable.

**Verification.**
- Unit test in `tests/wave1_nonlinear_tests.rs`: a model
  `I(p,n) <+ is_sat * (exp(V(p,n) / $vt) - 1.0)` at `T=350K` must produce a
  residual consistent with `vt = k*350/q ≈ 0.03016`, not the old `0.025852`.
- Test that `$temperature` reads as 350.0 when `SimCtx.temperature = 350.0`.
- Test that `$abstime` reads as the current solver time in a transient
  residual call.

**Acceptance criteria.**
- [ ] `SimCtx` struct defined `#[repr(C)]` and threaded through all 4 JIT fns.
- [ ] `Temperature`, `Abstime`, `Mfactor`, `Vt`, `Simparam` emit real values.
- [ ] No `_ => f64const(0.0)` reachable for accepted `SimQuery` variants.
- [ ] `validate_ir_contrib` still rejects the unimplemented variants
      (`XPosition`, `YPosition`, `Angle`, `ParamGiven`, `PortConnected`,
      `Limit`, `Random`) with `CodegenError::Unsupported`.
- [ ] `cargo test -p piperine-codegen` green.

---

### A.3 `SimQuery::Vt(temp)` emits a fixed constant, ignoring its argument

**Spec:** §8.1 — `$vt` is the thermal voltage `kT/q`, optionally parameterised
by a temperature argument.

**Status:** WRONG-CODE. **Critical** (wrong for any non-300K simulation).

**Current state.** `ir_emit.rs:139-142`:

```rust
SimQuery::Vt(_) => f64const(0.025852),   // "a usable constant default"
```

The argument is discarded; the value is hardcoded for `T = 300 K`.

**Why it matters.** Any diode/BJT model using `$vt` at a non-300K temperature
is silently wrong. The spec's `thermal_voltage(t)` function in Appendix A is
the explicit workaround, but `$vt` itself must work.

**Proposed solution.** Fixed by A.2's `SimCtx`. Specifically:
- `Sim(Vt(None))` → `(*sim).temperature * 8.617333262e-5` (Boltzmann constant
  in eV/K × T, giving V at q=1).
- `Sim(Vt(Some(t_arg)))` → evaluate `t_arg` (which must itself be a valid
  contrib expr — typically a `Param` or `Real`) and multiply by
  `8.617333262e-5`.

Use the CODATA value `k/q = 8.617333262e-5 V/K`.

**Decision rationale.** See A.2. The constant `0.025852` is removed.

**Verification.** See A.2 — the diode test at `T=350K` covers this.

**Acceptance criteria.**
- [ ] No hardcoded `0.025852` in `ir_emit.rs`.
- [ ] `$vt` at `T=300K` reads `≈ 0.025852` (regression-safe).
- [ ] `$vt` at `T=350K` reads `≈ 0.03016`.
- [ ] `$vt(T_param)` uses the argument.

---

### A.4 Digital interpreter: `Pow`/`Shl`/`Shr`/`AShl`/`AShr` silently become `Add`

**Spec:** §6.1, §8.3 — operators on discrete types must work; `**` is `Pow`,
`<<`/`>>` are shifts.

**Status:** WRONG-CODE. **Critical.**

**Current state.** `crates/piperine-codegen/src/ir_digital_to_interp.rs:154-156`:

```rust
IrBinOp::Pow | IrBinOp::Shl | IrBinOp::Shr | IrBinOp::AShl | IrBinOp::AShr => {
    BinaryOp::Add   // comment says "approximate as the left operand" but emits Add
}
```

A digital guard like `if (x ** 2 > 10)` or `if (x << 4 == 0)` silently becomes
`if (x + 2 > 10)` / `if (x + 4 == 0)`. The comment and the code disagree.

**Why it matters.** Digital state machines that use shifts (common in CRC,
bitfield manipulation) or `**` compile to wrong logic with no warning.

**Proposed solution.** Two steps:

1. **Fail-loud now:** reject these operators in the digital path with a clear
   error, so users are not lied to. Add a validation pass at the entry of
   `ir_digital_to_interp` (or a `validate_ir_digital` function analogous to
   `validate_ir_contrib`).

   ```rust
   // new: src/codegen/digital_validate.rs
   pub fn validate_ir_digital(e: &IrExpr) -> Result<(), CodegenError> {
       match e {
           IrExpr::Binary(op, a, b) => match op {
               IrBinOp::Pow | IrBinOp::Shl | IrBinOp::Shr
               | IrBinOp::AShl | IrBinOp::AShr => {
                   return Err(unsupported(format!("operator {op:?} in digital block")));
               }
               _ => {}
           },
           // recurse into a, b, sub-expressions ...
           _ => {}
       }
       Ok(())
   }
   ```

2. **Implement properly later:** add `Pow`/`Shl`/`Shr` to the digital
   interpreter's eval (`codegen/digital.rs:398-461`). For `Natural`/`Integer`:
   `<<` / `>>` are `<<` / `>>` on the integer; `**` is `pow` (with integer
   exponent). For `Quad`/`Boolean`, fall back to fail-loud (bit semantics on
   4-state are subtle).

**Decision rationale.** Fail-loud first, exactly as Part A's principle. The
full 4-state bitwise semantics can wait; the wrong-`Add` cannot.

**Verification.** Negative-assertion test in
`tests/ir_digital_to_interp_tests.rs`:

```rust
#[test]
fn shift_in_digital_guard_is_rejected_not_silently_add() {
    let ir = build_digital_ir_with_shift_guard();
    let err = ir_digital_to_interp(&ir, "shift_fsm").unwrap_err();
    assert!(err.to_string().contains("Shl") || err.to_string().contains("shift"),
        "expected shift rejection, got: {err}");
}
```

**Acceptance criteria.**
- [ ] `BinaryOp::Add` fallback for `Pow/Shl/Shr/AShl/AShr` removed.
- [ ] `validate_ir_digital` rejects these ops with a clear message.
- [ ] Negative-assertion test passes.
- [ ] `cargo test -p piperine-codegen` green.

---

### A.5 Digital interpreter: all non-`Neg` unary ops silently become `Not`

**Spec:** §6.1 — `BitNot` (`~`), reduction ops (`&`, `|`, `^`, `~&`, `~|`, `~^`).

**Status:** WRONG-CODE. **High.**

**Current state.** `ir_digital_to_interp.rs:167`:

```rust
IrUnOp::Not | IrUnOp::BitNot | IrUnOp::RedAnd | IrUnOp::RedNand
| IrUnOp::RedOr | IrUnOp::RedNor | IrUnOp::RedXor | IrUnOp::RedXor => {
    UnaryOp::Not   // all collapse to logical NOT
}
```

`~x` (bitwise NOT) and `&x` (reduction AND) both become `!x` (logical NOT),
which on a multi-bit value is wrong.

**Why it matters.** Reduction operators are common in digital (priority
encoders, parity). Silently mapping to `!` gives wrong control flow.

**Proposed solution.** Same two-step pattern as A.4:

1. **Fail-loud now:** in `validate_ir_digital`, reject `BitNot` and all
   reduction ops with `unsupported("unary operator {op:?} in digital block")`.
2. **Implement later:** add them to `codegen/digital.rs` eval. Reduction `&x`
   on a `Natural(n)` → `n == ((1 << bitwidth) - 1)` (all-ones); this requires
   knowing the bitwidth, which means the digital interpreter needs width
   metadata (today it does not — see I.6).

**Decision rationale.** Reductions need width, which needs type checking (Part
B). Defer the implementation; do the fail-loud rejection now.

**Verification.** Negative-assertion test as in A.4.

**Acceptance criteria.**
- [ ] `UnaryOp::Not` fallback for non-`Neg`/`Not` ops removed.
- [ ] `validate_ir_digital` rejects `BitNot` and reductions.
- [ ] Negative-assertion test passes.

---

### A.6 `from_ir` silently swallows child compile errors via `.ok()`

**Spec:** §5.3 — every instance becomes a device; a missing device is a
hard error, not a silent skip.

**Status:** WRONG-CODE / silent skip. **High.**

**Current state.** `crates/piperine-codegen/src/from_ir.rs:146-147, 153,
175-186`:

```rust
let analog_dev = ir_analog_to_device(prog, &inst.module).ok();
let digital_interp = ir_digital_to_interp(prog, &inst.module).ok();
// ...
if analog_dev.is_some() || digital_interp.is_some() {
    // ... build device ...
} else {
    // silently skipped — no error propagated
}
```

If a child's analog body fails to compile (e.g. `vsource.va`'s ideal `V <+`,
which is currently unsupported per D.1), the child becomes `None`, and if it
has no digital body either it is silently dropped. The netlist is then
incomplete with no diagnostic.

**Why it matters.** A user's circuit silently loses devices. This is the
single biggest "why doesn't my simulation match expectations" trap.

**Proposed solution.** Replace `.ok()` with proper error propagation and a
"no body" distinction:

```rust
let analog_dev = match ir_analog_to_device(prog, &inst.module) {
    Ok(d) => Some(d),
    Err(CodegenError::NoAnalogBody) => None,   // module has no analog block — fine
    Err(e) => return Err(format!(
        "instance `{}` of module `{}` failed to compile analog body: {e}",
        inst.label, inst.module
    )),
};
// same pattern for digital
```

Add `CodegenError::NoAnalogBody` / `NoDigitalBody` variants for the "module
genuinely has no such block" case (which is not an error). The
`ir_analog_to_device`/`ir_digital_to_interp` functions must return these
distinctly rather than a generic error when `body.is_none()`.

**Decision rationale.** Errors must propagate. The only non-error case is
"this module has no body of that kind", which deserves its own variant.

**Verification.** Test in `tests/from_ir_tests.rs`: a top module instantiating
a child with an unsupported analog construct must return `Err(...)` with the
child's name in the message, not an empty device list.

```rust
#[test]
fn from_ir_propagates_child_compile_error_not_silent_skip() {
    let ir = build_ir_with_unsupported_child();
    let err = from_ir(&ir, "top").unwrap_err();
    assert!(err.contains("u1"), "error should name the instance: {err}");
    assert!(err.contains("vsource"), "error should name the module: {err}");
}
```

**Acceptance criteria.**
- [ ] No `.ok()` on `ir_analog_to_device`/`ir_digital_to_interp` in `from_ir`.
- [ ] `CodegenError::NoAnalogBody` / `NoDigitalBody` added and used.
- [ ] Error message includes instance label and module name.
- [ ] Test passes; existing tests still green (children with no body still
      work — they just have `None`/`None` and are skipped legitimately).

---

### A.7 `from_elab` analog path silently stamps `ddt` as 0

**Spec:** §8.1 — `ddt` is a core analog operator.

**Status:** WRONG-CODE / silent zero. **High.**

**Current state.** `crates/piperine-codegen/src/codegen/analog.rs:188-190`:

```rust
// The PHDL `from_elab` path does not extract reactive charge contributions
// yet; `ddt` there stamps as 0 (DC) as before.
react_contributions: Vec::new(),
```

The `from_elab` path (PHDL `Expr` → JIT, via `autodiff.rs`) builds no
reactive contributions. A capacitor compiled through `from_elab` has no
charge term and behaves like an open circuit in transient.

**Why it matters.** There are two parallel analog paths (`from_elab` and
`compile_analog_module_ir`). Only the IR path supports `ddt`. Anything still
using `from_elab` silently loses capacitance.

**Proposed solution.** See K.1 — the strategic decision is to **deprecate
`from_elab` and route everything through the IR path**. In the meantime:

1. Audit call sites of `compile_analog_module` (the `from_elab` entry). Per
   the explore report, the only callers are tests and `from_elab.rs` itself.
2. Add a `validate_*` guard to `compile_analog_module` that rejects any
   `Expr::Call("ddt", _)` / `Expr::Call("idt", _)` with a clear error, so the
   silent zero becomes a loud error.
3. Migrate the remaining callers to `compile_analog_module_ir` via `ppr_to_ir`
   + `ir_analog_to_device`.

**Decision rationale.** Two paths is a liability. The IR path is the
strategic future (it has validation, reactive support, and is the contract
with both frontends). Keep `from_elab` only until callers migrate, and make
it fail-loud in the meantime.

**Verification.** Add a test that `compile_analog_module` (from_elab) on a
capacitor returns `Err` mentioning `ddt`, not a device that silently has no
charge.

**Acceptance criteria.**
- [ ] `compile_analog_module` rejects `ddt`/`idt` with an explicit error.
- [ ] No silent `0.0` for `ddt` in `expr.rs`.
- [ ] All existing capacitor tests route through the IR path.
- [ ] `cargo test -p piperine-codegen` green.

---

### A.8 `Param`/`Var` unresolved names silently read as 0 in analog JIT

**Spec:** §7 — a `fn`'s parameters and a module's `param`s are read by name;
an unresolved name is a compile error, not a silent 0.

**Status:** WRONG-CODE / silent zero. **High.**

**Current state.** `ir_emit.rs:90-93`:

```rust
IrExpr::Param(name) | IrExpr::Var(name) => {
    if let Some(&v) = param_values.iter().find(|(n, _)| n == name).map(|(_, v)| v) {
        builder.ins().load(f64, MemFlags::trusted(), v, 0)
    } else {
        f64const(0.0)   // <-- unresolved name silently 0
    }
}
```

A local `var` declared inside an analog body (which is not in the module's
`params` list) reads as 0. A typo in a param name reads as 0.

**Why it matters.** Wrong results with no diagnostic.

**Proposed solution.** Move this check into `validate_ir_contrib`: collect the
set of valid `Param`/`Var` names from the module's `params` and `analog.vars`
(`IrAnalogBody.vars: Vec<IrVarDecl>`), and reject any `Param(name)`/`Var(name)`
not in that set with `unsupported("unresolved name `{name}` in analog body")`.

```rust
// in validate_ir_contrib, thread a `&HashSet<String>` of known names:
IrExpr::Param(name) | IrExpr::Var(name) => {
    if known_names.contains(name) {
        Ok(())
    } else {
        Err(unsupported(format!("unresolved name `{name}` in analog body")))
    }
}
```

The emitter then keeps a `debug_assert!` or a panic for the defensive case
(reaching the emitter with an unvalidated name is a programming bug).

**Decision rationale.** Names must resolve at validation, not silently
default. This matches the fail-loud discipline.

**Verification.** Test that a model with a typo'd param name fails with
"unresolved name `r_typo`".

**Acceptance criteria.**
- [ ] `validate_ir_contrib` takes (or wraps) a known-names set.
- [ ] Unresolved `Param`/`Var` rejected.
- [ ] Emitter's `f64const(0.0)` for unresolved names is unreachable post-validation.
- [ ] Test passes.

---

### A.9 `V(a,b)` with unknown terminal names silently reads as 0

**Spec:** §8.1 — `V(a, b)` reads the branch between two named nets.

**Status:** WRONG-CODE / silent zero. **Medium-High.**

**Current state.** `codegen/analog.rs:401, 405`: if `plus` or `minus` is not
in `port_index`, the voltage contribution silently reads as 0.

**Why it matters.** A typo in a terminal name (`V(p, gnd)` where the net is
`ground`) silently reads 0 instead of erroring.

**Proposed solution.** Validate terminal names in `validate_ir_contrib` (or a
sibling `validate_branches`) against the module's port + wire + ground names.
Reject unknown names with `unsupported("unknown terminal `{name}`")`.

**Decision rationale.** Same as A.8 — names must resolve.

**Verification.** Test that `V(p, nonexistent)` fails validation.

**Acceptance criteria.**
- [ ] Unknown terminal names rejected at validation.
- [ ] Test passes.

---

### A.10 AMS preprocessor: `` `elsif `` is broken

**Spec:** N/A (Verilog-AMS standard preprocessor).

**Status:** WRONG-CODE / silent misparse. **Medium.**

**Current state.** `crates/piperine-ams/src/preprocessor.rs:98-121` — the
dispatch match has no `` `elsif `` arm, so `` `elsif X `` falls into the
catch-all and is treated as a macro *use*, erroring with "undefined macro
`elsif`". The formatter's `DirectiveRule` mentions `` `elsif `` (`fmt.rs:174,
187`) but the preprocessor that drives parsing does not handle it.

**Why it matters.** Any AMS file using `` `elsif `` fails to parse with a
confusing error.

**Proposed solution.** Add an `` `elsif `` arm to the preprocessor dispatch.
`` `elsif X `` is equivalent to `` `else `ifdef X `` — implement it by
flipping the current branch's `active` state only if no prior branch in this
`` `ifdef `` chain has been `taken`, and consulting `X`.

Concretely, extend the `IfdefState` stack entry to track `taken_in_chain:
bool` and on `` `elsif ``:
- if `taken_in_chain` is already true → this branch is inactive, push false.
- else → evaluate `X`; if defined, this branch is active and mark
  `taken_in_chain = true`; else inactive.

**Decision rationale.** Standard CPP semantics. The existing `` `ifdef `` /
`` `else `` / `` `endif `` framing (`preprocessor.rs:245-267`) already tracks
`parent_active`/`taken`/`active`; extend it with a chain-taken flag.

**Verification.** Add a fixture `crates/piperine-ams/tests/fixtures/vams/elsif_test.vams`:

```verilog
`ifdef A
  // branch A
`elsif B
  // branch B
`else
  // branch C
`endif
```

And a test in `tests/suite_test.rs` (or a new `tests/preprocessor_test.rs`)
that parses it with `B` defined and asserts the branch-B content is retained.

**Acceptance criteria.**
- [ ] `` `elsif `` arm present in preprocessor dispatch.
- [ ] `taken_in_chain` (or equivalent) tracked across the if/elsif/else chain.
- [ ] New fixture + test pass.
- [ ] No regression in `cargo test -p piperine-ams`.

---

### A.11 AMS 4-state sized literals silently become 0

**Spec:** N/A (Verilog-AMS literal syntax).

**Status:** WRONG-CODE / silent zero. **Medium.**

**Current state.** `crates/piperine-codegen/src/from_ams.rs:1126-1144` —
`parse_sized_lit` uses `i64::from_str_radix`, which rejects `x`/`X`/`z`/`Z`/`?`
digits. The lexer accepts them (`lexer.rs:303-312`) but IR conversion silently
returns 0.

**Why it matters.** `4'b1x0z` (a common don't-care pattern) becomes 0 with no
warning.

**Proposed solution.** Two options:

1. **Fail-loud:** if the sized literal contains `x`/`z`/`?`, return
   `IrExpr::Quad(...)` with the right per-bit encoding (0=0, 1=1, 2=X, 3=Z)
   instead of forcing to `i64`. This requires the IR to carry 4-state values
   for sized literals, which it already can (`IrExpr::Quad(u8)` exists but
   only holds a single 2-bit value — extend to a `Vec<QuadBit>` or a
   `QuadWord` if multi-bit).
2. **Minimal fail-loud:** if `from_str_radix` fails, emit a clear error
   `"4-state sized literal `{lit}` not yet supported in IR"` rather than 0.

**Decision rationale.** Adopt Option 2 now; track Option 1 under Part I (4-state
type work). The full multi-bit Quad representation is a larger change.

**Verification.** Test that `4'b1x0z` produces an error mentioning "4-state",
not a silent 0.

**Acceptance criteria.**
- [ ] No silent 0 for 4-state sized literals.
- [ ] Error message names the literal and the cause.
- [ ] Test passes.

---

### A.12 `Truncation.rs:81` panics on `Gear { order: 7 }`

**Spec:** N/A (internal).

**Status:** Violates "no panic on user input" rule. **Low** (enum is unused).

**Current state.** `crates/piperine-solver/src/analysis/truncation.rs:81`:

```rust
panic!("Gear method order must be between 1 and 6")
```

**Proposed solution.** Return a `Result` or clamp. Since the enum is unused,
the cleanest fix is to make `truncation_coefficient` return `Result<f64, ...>`
and have the (currently dead) callers handle it. Alternatively, clamp to 6
with a `debug_assert!`. Prefer the `Result` form for forward-compatibility.

**Decision rationale.** AGENTS.md rule. Even unused code should not panic on
user-constructible input.

**Verification.** The existing `should_panic` test at `truncation.rs:181-185`
must be flipped to a `Result`-checking test.

**Acceptance criteria.**
- [ ] No `panic!` on `Gear { order: 7 }`.
- [ ] Existing test updated.
- [ ] `cargo test -p piperine-solver` green.

---

### A.13 `TransferFunctionSolver` debug `eprintln!` in production

**Spec:** N/A.

**Status:** Noise / unprofessional. **Low.**

**Current state.** `crates/piperine-solver/src/solver/tf.rs:68-71` and `:98-102`
print the entire DC operating-point map on every `TransferFunctionSolver::new`.

**Proposed solution.** Remove, or gate behind `tracing::debug!`. The crate
already depends on `tracing` (see `Cargo.toml` workspace deps).

**Acceptance criteria.**
- [ ] No `eprintln!` in `tf.rs`.
- [ ] `tracing::debug!` gated version optional.
- [ ] `cargo test -p piperine-solver` green.

---

### A.14 `$simparam("gmin", d)` and other solver params rejected at codegen

**Spec:** Verilog-A/AMS standard — `$simparam("name", default)` reads a
named solver parameter, returning `default` if unknown. Critical for
junction diodes (gmin regularisation) and waveform defaults (step/tfinal).

**Status:** WRONG-CODE / codegen reject. **High.**

**Cross-reference:** `docs/NGSPICE_GAPS.md` N.1 — affects every NGSPICE
semiconductor model in `crates/piperine-lang/headers/ngspice/`.

**Current state.** The lowering produces
`IrExpr::Sim(SimQuery::Simparam { key, default })`
(`crates/piperine-lang/src/lowering/expr.rs:437-445`). The JIT
validator rejects every `SimQuery` other than `Temperature | Vt | Abstime |
Mfactor` at `ir_emit.rs:526-531`. The `SimCtx` already carries a `gmin:
f64` field at offset 24 (`codegen/mod.rs:67-69`) but **nothing reads it**.

A diode model written
`I(pp, n) <+ cd + gmin * vd;` parses, lowers, and then fails to compile
with `CodegenError::Unsupported("SimQuery::Simparam ...")`. Every
semiconductor in the NGSPICE faithful set (`dio`, `bjt`, `jfet`, `mos1`)
reaches for gmin; without it, junction conductances are zero at the
operating point and the matrix is singular.

**Why it matters.**
1. **DC convergence:** SPICE's `gmin` (default 1e-12 S) is added to every
   pn junction to guarantee a non-singular matrix. Without it, a "floating"
   node (no DC path to ground) has zero row sum and the Newton iteration
   diverges.
2. **Source ramping:** `vsrc`/`isrc` use `$simparam("step", 1e-6)` and
   `$simparam("tfinal", 1e-3)` to default their `TR`/`TF`/`PW`/`PER`
   waveform coefficients. Without these, every transient simulation
   requires the user to repeat SPICE defaults manually.
3. **Solver plumbing exists** (`SimCtx.gmin` is laid out, just unused);
   this is wiring, not design.

**Proposed solution.** Two parts:

```rust
// codegen/ir_emit.rs:121-170 — extend `emit_sim` with:
SimQuery::Simparam { key, default } => {
    match key.as_str() {
        "gmin"   => load_offset(ctx.sim_ctx, 24),          // already there
        "step"   => load_offset(ctx.sim_ctx, /* new slot */),
        "tfinal" => load_offset(ctx.sim_ctx, /* new slot */),
        other    => emit_ir_expr(ctx, default),           // fall back
    }
}
```

1. Add new fields to `SimCtx` (`codegen/mod.rs:56-82`):
   ```rust
   pub step: f64,       // transient step (set by solver before each call)
   pub tfinal: f64,     // transient final time
   ```
   with a `#[repr(C)]` layout (follows A.2's pattern).

2. The solver sets these once per `load_transient` call (no per-step cost).

3. The `validate_ir_contrib` / `emit_ir_expr` reject the unknown-key path with
   a clear error rather than silently returning the default — fail-loud per
   Part A's principle.

**Decision rationale.** Use the existing `SimCtx` infra rather than a new
syscall category. gmin is already laid out there; the others are one field
each. The alternative — a `HashMap<String, f64>` per device — is more
flexible but costs a lookup per `$simparam` call and adds allocation.
Given the known four keys (`gmin`, `step`, `tfinal`, plus a `temper` alias
for `$temperature`), a struct is the right granularity.

**Verification.**
- Unit test in `tests/wave1_nonlinear_tests.rs` (positive-assertion):
  ```rust
  // Simple PHDL model: `I(p,n) <+ cd + gmin * vd;`
  // After A.14: codegen succeeds, residual includes gmin * vd term.
  // With gmin=1e-12, V(p,n)=1.0, current ≈ 1e-12.
  ```

- Failure path: `$simparam("nonexistent", 0.5)` lowers but codegen reports
  `"unknown simparam `nonexistent`; falling back to default 0.5"` so users
  know the lookup miss happened.

- Integration: `dio` model from `crates/piperine-lang/headers/ngspice/`
  compiles end-to-end (codegen succeeds; the resulting device's residual
  is finite). Until A.15 + I.14 also land, this is a partial test, but it
  validates just the `$simparam` codegen piece.

**Acceptance criteria.**
- [ ] `SimQuery::Simparam` accepted by `emit_sim` for keys `gmin`, `step`,
      `tfinal`, `temperature`.
- [ ] Unknown keys fall back to the `default` argument with a clear log.
- [ ] `SimCtx` carries `step` and `tfinal`; solver sets them at
      `load_transient`.
- [ ] `ngspice_headers_parse_tests` (or new
      `ngspice_compile_tests`) asserts the four NGSPICE semiconductor models
      lower to `emit_ir_expr` (codegen succeeds; no silent failure).
- [ ] `cargo test -p piperine-codegen -p piperine-solver` green.

---

### A.15 `$param_given("name")` rejected at codegen

**Spec:** Verilog-A/AMS standard — returns 1 if the named instance
parameter was explicitly set, 0 otherwise. Foundation for SPICE's
"instance overrides model" parameter resolution.

**Status:** WRONG-CODE / codegen reject. **High.**

**Cross-reference:** `docs/NGSPICE_GAPS.md` N.2; affects every NGSPICE
semiconductor model in `crates/piperine-lang/headers/ngspice/` (they all
use `$param_given("…")` to decide between instance and model defaults).

**Current state.** The lowering produces
`IrExpr::Sim(SimQuery::ParamGiven(name))` (`lowering/expr.rs:446-453`).
The JIT validator rejects it (`ir_emit.rs:526-531`).

Today, models work around the gap with a sentinel convention:
`param temp : Real = 0.0;` means "not given, use ambient". This is fragile —
`0.0 K` is physically valid in the math, even if not in practice — and
breaks for parameters like `nf`/`nr` which have `1.0` as a meaningful
default.

**Why it matters.** SPICE's `.MODEL XRX resmod(r=50 tc1=0.001)` then 100
instances `R1 : res ( a, b )` (inheriting model) vs `R1 : res ( a, b ) {
.r = 100 }` (overriding) is the central reason for the model/instance
separation. Without `$param_given`, every NGSPICE faithful model needs a
sentinel, which the faithful headers already use (`temp > 0.0`), but
parameters like `nf`, `nr`, `ise`, `isc`, `rb`, `irb`, etc. do **not**
have safe sentinel defaults — they're exactly the values you'd expect
to be set differently per instance.

**Proposed solution.** Two parts:

1. **At elaboration time**, when a `param` on an instance is bound (`R ( a, b
) { .r = 50.0 }` → instance binds the `r` slot), record
`param_given: HashMap<String, usize>` on `ElabMod` (instance → bitmask
index). Conceptually: each instance has a fixed-size bool vector, one
bit per declared `param`, set to 1 if the parent supplied a value.

2. **At codegen time**, extend `Device` to expose a per-instance
`param_given: Arc<[bool]>` and add `emit_ir_expr` for
`SimQuery::ParamGiven(name)`:
   ```rust
   SimQuery::ParamGiven(name) => {
       let i = ctx.param_given
           .iter()
           .position(|k| k == name)
           .ok_or_else(|| unsupported(format!(
               "unknown param `{name}` in $param_given"))?;
       if ctx.param_given_values[i] { f64const(1.0) } else { f64const(0.0) }
   }
   ```

3. The instance param storage already knows which param names are
   declared in the module — the `param_given` bitmask is indexed by
   declaration order (matching `IrProgram.modules[i].params[j]`).

**Decision rationale.** A bool bitmask is cheaper than a `HashMap` lookup
per `$param_given` call (the call can appear in inner loops). Reusing the
existing `Device` storage layout keeps the codegen simple.

**Verification.**
- Positive: `R ( a, b ) { .r = 50.0 }` with `var x : Real = if
  ($param_given("r")) { 50.0 } else { 0.0 }` (a fake "watch the
  param-given flag" model) produces `x = 50.0` when bound, `x = 0.0`
  when not.
- Negative: `$param_given("r_typo")` errors with `"unknown param
  `r_typo` in $param_given"` (fail-loud per Part A).
- Integration: NGSPICE faithful passives.phdl `res` model resolves `r`
  vs `model.r` correctly. Use `cargo test -p piperine-codegen`.

**Acceptance criteria.**
- [ ] Elaboration populates `param_given: &[bool]` per instance.
- [ ] `SimQuery::ParamGiven` accepted by `emit_sim`; reads from
      `param_given` bitmask.
- [ ] Unknown param name rejected with a clear error.
- [ ] NGSPICE res model E2E test: instance `r=50` → uses 50; instance
      with no `r` → uses `model.r` (or 1 mΩ fallback).
- [ ] `cargo test -p piperine-codegen` green.

---

## Part B — Type system & the no-magic rule

> The spec's central promise (§6: "well-formed by construction", §10:
> "no-magic") is unmet today: there is **no type checking at all** in
> `piperine-lang`. This Part adds the minimum type system needed to make the
> spec's promises true, without building a full type inference engine.

### B.1 Add a typed-elaboration pass: width matching

**Spec:** §6.1 — `Bit[8]` connected to `Bit[4]` is a compile error.

**Status:** MISSING. **Critical.**

**Current state.** `crates/piperine-lang/src/elab/lower.rs:509-513` reduces
both sides of a connection to `ElabNetRef` without comparing disciplines or
widths. A `Bit[8]` port connected to a `Bit[4]` wire elabora silently.

**Why it matters.** Wrong net widths produce silently-wrong fan-out. This is
the most common class of HDL bug and the spec explicitly promises to prevent
it.

**Proposed solution.** Add a `typecheck` pass between `elaborate` and
`ElabProgram` emission. Concretely:

1. **Extend `ElabNetType`** (`elab/ir.rs`) to carry a width:
   ```rust
   pub enum ElabNetType {
       Discipline { name: String, width: usize },   // Electrical, Electrical[8], ...
       Bundle    { name: String, width: usize },    // net-capable bundle
   }
   ```
   Today `ElabNetType` likely has no width; add it. The width comes from the
   array dimensions in `wire node : Electrical[N+1]` (already const-evaled at
   `lower.rs:139-155`).

2. **Add `typecheck/mod.rs`** under `piperine-lang/src/`:
   ```rust
   pub fn typecheck(prog: &ElabProgram) -> Result<(), ElabError> {
       for module in prog.modules.values() {
           check_module(module, prog)?;
       }
       Ok(())
   }

   fn check_module(m: &ElabMod, prog: &ElabProgram) -> Result<(), ElabError> {
       for conn in &m.connections {
           let lhs_ty = resolve_net_ty(&conn.lhs, m)?;
           let rhs_ty = resolve_net_ty(&conn.rhs, m)?;
           if lhs_ty.width() != rhs_ty.width() {
               return Err(ElabError::TypeMismatch {
                   lhs: conn.lhs.clone(), rhs: conn.rhs.clone(),
                   reason: format!("width mismatch: {} vs {}", lhs_ty.width(), rhs_ty.width()),
               });
           }
           // discipline check — see B.2
       }
       for inst in &m.instances {
           check_instance_connections(inst, m, prog)?;
       }
       Ok(())
   }
   ```

3. **Wire it into the pipeline** (`lib.rs::parse_and_elaborate`):
   ```rust
   pub fn parse_and_elaborate(input: &str) -> Result<ElabProgram, String> {
       let source = parse_str(input)?;
       let prog = elaborate(source).map_err(|e| e.to_string())?;
       typecheck::typecheck(&prog).map_err(|e| e.to_string())?;
       Ok(prog)
   }
   ```

**Decision rationale.** A separate `typecheck` pass (post-elaboration,
pre-codegen) is cleaner than threading type state through every `lower.rs`
arm. It can be turned on incrementally (start with width, add discipline
checking next). The `ElabError::TypeMismatch` variant is new.

**Verification.** Tests in a new `tests/typecheck_tests.rs`:

```rust
#[test]
fn width_mismatch_is_caught() {
    let src = "
        discipline Bit { storage Boolean; }
        mod A ( input x : Bit[8] );
        mod Top { wire a : Bit[4]; u1 : A ( a ); }
    ";
    let err = piperine_lang::parse_and_elaborate(src).unwrap_err();
    assert!(err.contains("width") && err.contains("8") && err.contains("4"));
}

#[test]
fn width_match_passes() {
    let src = "
        discipline Bit { storage Boolean; }
        mod A ( input x : Bit[8] );
        mod Top { wire a : Bit[8]; u1 : A ( a ); }
    ";
    assert!(piperine_lang::parse_and_elaborate(src).is_ok());
}
```

**Acceptance criteria.**
- [ ] `ElabNetType` carries width.
- [ ] `typecheck` pass exists and is called by `parse_and_elaborate`.
- [ ] Width mismatch produces `ElabError::TypeMismatch`.
- [ ] Tests pass; existing elaboration tests still green.

---

### B.2 Discipline-crossing connections are rejected (no-magic rule)

**Spec:** §10 — "Connecting incompatible disciplines is a compile error".

**Status:** MISSING. **Critical.**

**Current state.** No discipline comparison anywhere in `lower.rs` connection
handling.

**Why it matters.** This is the spec's headline rule. Without it, a
`Thermal` net can connect to an `Electrical` net and the simulator has no
idea the physics is wrong.

**Proposed solution.** In the `typecheck` pass from B.1, after the width
check, compare disciplines:

```rust
fn check_discipline_compat(lhs: &ElabNetType, rhs: &ElabNetType) -> Result<(), ElabError> {
    let l = lhs.discipline_name();
    let r = rhs.discipline_name();
    if l == r {
        return Ok(());
    }
    // Ground is compatible with any conservative discipline's reference
    if l == "Ground" || r == "Ground" {
        return Ok(());
    }
    Err(ElabError::TypeMismatch {
        lhs: l.to_owned(), rhs: r.to_owned(),
        reason: format!("discipline crossing `{l}` ↔ `{r}` requires an explicit converter module (§10)"),
    })
}
```

Ground special-casing: the spec says `Ground` is predefined and fixed at
zero; connecting `gnd` to an `Electrical` reference net is the normal way to
ground a node, so `Ground` is compatible with any conservative discipline.

**Decision rationale.** The rule is "incompatible disciplines need an
explicit converter mod". Ground is the one exception (it's the universal
reference). Bundle-typed nets check discipline recursively per field (B.3).

**Verification.** Test:

```rust
#[test]
fn discipline_crossing_is_rejected() {
    let src = "
        discipline Electrical { potential v : Real; flow i : Real; }
        discipline Thermal { potential temp : Real; flow pwr : Real; }
        mod A ( inout e : Electrical, inout t : Thermal );
        mod Top { wire e : Electrical; wire t : Thermal; u1 : A ( e, t ); }
        // connecting e to t directly:
        mod Bad ( inout e : Electrical, inout t : Thermal ) { e = t; }
    ";
    let err = piperine_lang::parse_and_elaborate(src).unwrap_err();
    assert!(err.contains("discipline crossing"));
    assert!(err.contains("Electrical") && err.contains("Thermal"));
}
```

**Acceptance criteria.**
- [ ] Discipline mismatch produces `ElabError::TypeMismatch` with the §10 message.
- [ ] `Ground` ↔ any conservative discipline is allowed.
- [ ] Same-discipline connections still pass.
- [ ] Tests pass.

---

### B.3 Bundle-typed connections check field-by-field

**Spec:** §6.5 — "Two nets of the same bundle type connect field-by-field by
name."

**Status:** PARTIAL. **High.**

**Current state.** Port expansion works (`lower.rs:288-319`), but the
*connection side* does not fan out — `eval_net_ref` only handles
`Ident.field` → `{base}_{field}` (`lower.rs:267-278`). A bare bundle ident on
the connection side reduces to `ElabNetRef::simple("a")` without field
fanout.

**Proposed solution.** Two changes:

1. In `eval_net_ref`, when the base is a bare `Ident(name)` and `name` is a
   known bundle-typed wire/port in the current module, expand to a
   multi-field reference. This requires `eval_net_ref` to return
   `Vec<ElabNetRef>` (or a new `ElabNetRef::Bundle(name, Vec<field>)`).
2. In the typecheck pass, for a `bundle1 = bundle2` connection, look up the
   bundle declaration (which requires exposing `BundleDecl` in `ElabProgram`
   — see K.3), check that the two bundles are the same type, and emit one
   field-by-field discipline/width check per field.

**Decision rationale.** The spec says bundle-bundle connection is by name
per field. The current `Ident.field` flattening works for single-field
references but not for whole-bundle passes. Exposing `BundleDecl` in
`ElabProgram` is prerequisite (K.3).

**Verification.** Test that a `DiffPair` connects to a `DiffPair` port
field-by-field, and that a `DiffPair` connecting to a non-DiffPair bundle is
rejected.

**Acceptance criteria.**
- [ ] Bundle-to-bundle connection fans out per field.
- [ ] Mismatched bundle types rejected.
- [ ] `BundleDecl` exposed in `ElabProgram` (K.3 done first).
- [ ] Tests pass.

---

### B.4 Single-driver enforcement for `output` and single-driver nets

**Spec:** §6.3 — "Single-driver is the default for signal-flow and digital
nets; a second driver is an error."

**Status:** MISSING. **High.**

**Current state.** No driver-counting anywhere.

**Why it matters.** Two drivers on a single-driver net is a classic error
that simulators usually catch at runtime; the spec promises to catch it at
compile time.

**Proposed solution.** In the typecheck pass, for each net in each module,
count the number of drivers. A "driver" is:
- An `output` port of a child instance connected to that net.
- A `<-` force (analog) on that net in the module's own `analog` block.
- A `<-` drive (digital) on that net in the module's own `digital` block.
- A `=` continuous assignment to that net.

For a `discipline` with `resolve` clause (tri/or/and), multiple drivers are
allowed. For `Bit` (storage `Boolean`), `Bit` is single-driver only — the
spec is explicit. For `output` direction, always single-driver.

```rust
fn count_drivers(net: &str, m: &ElabMod, prog: &ElabProgram) -> usize {
    let mut n = 0;
    for inst in &m.instances {
        for conn in &inst.connections {
            if conn.net == net && conn.port_direction() == Direction::Out {
                n += 1;
            }
        }
    }
    // scan analog/digital bodies for `<-`/`<-`/`=` on this net
    n += count_drives_in_behaviors(net, m, prog);
    n
}
```

If `n > 1` and the net's discipline does not have a `resolve` clause →
`ElabError::MultipleDrivers { net, count: n }`.

**Decision rationale.** Driver counting is a static analysis over the
elaborated module. It needs behavior-body scanning, which means the typecheck
pass must see the `behaviors` (already in `ElabProgram`). Start with `output`
ports and `<-` drives; continuous assigns and forces can be added.

**Verification.** Test two `output` ports wired to the same net → error.

**Acceptance criteria.**
- [ ] `ElabError::MultipleDrivers` variant added.
- [ ] Two `output` drivers on one net rejected.
- [ ] `resolve tri` discipline allows multiple drivers.
- [ ] Tests pass.

---

### B.5 `Boolean` widens to `Quad` implicitly; other casts explicit

**Spec:** §6.1 — "`Boolean` widens to `Quad` implicitly; casts are otherwise
explicit (`real(x)`, `int(x)`, `bit(x)`)."

**Status:** MISSING. **Medium.**

**Current state.** No widening logic anywhere.

**Proposed solution.** In the typecheck pass, when a `Boolean` value is
used in a `Quad` context (e.g. assigned to a `Logic` net, or passed to a
`Quad` parameter), allow it with an implicit widen `0→0q0`, `1→0q1`. All
other type coercions require an explicit cast call (`real(x)`, `int(x)`,
`bit(x)`). Casts themselves need to be recognized (currently they're just
`Expr::Call(Expr::Ident("real"), [x])` — see J.4).

**Decision rationale.** The spec's one widening is the safe one (`Boolean`
has no `X`/`Z`, so widening is lossless). Everything else is potentially
lossy and must be explicit.

**Verification.** Test that `Boolean → Quad` assignment passes; `Real →
Integer` assignment without `int(...)` fails.

**Acceptance criteria.**
- [ ] `Boolean → Quad` implicit widen accepted.
- [ ] `Real → Integer` without cast rejected.
- [ ] Tests pass.

---

## Part C — Standard library & prelude

> The spec's examples (`gnd : Ground`, `UInt[N]`, `SInt[N]`) all assume a
> prelude that does not exist. This Part adds it.

### C.1 Predefine `Ground` discipline

**Spec:** §6.2 — "`Ground` is predefined, fixed at zero."

**Status:** MISSING. **Critical** (blocks every analog example).

**Current state.** No `Ground` in `src/stdlib/` or anywhere in `src/`. Every
example using `gnd : Ground` only parses; none elaborate.

**Proposed solution.** Add `Ground` as a built-in discipline, injected by the
resolver (`src/resolve/mod.rs:87-94`, which already injects `capabilities.phdl`
and `collections.phdl` via `include_str!`).

Two implementation options:

**Option A (special-case):** in `resolve_net_type` (`lower.rs:157-206`),
special-case the name `Ground` to return a synthetic `ElabNetType::Discipline
{ name: "Ground", width: 1 }` without requiring a discipline declaration. The
codegen (Part D, E) treats `Ground` nets as the implicit reference node
(already does — `from_ir.rs:42-43` recognises `gnd`/`GND`/`vss`/`VSS`).

**Option B (prelude file):** add `src/stdlib/ground.phdl`:
```phdl
discipline Ground {
    potential v : Real (unit = "V", abstol = 1e-6);
}
```
and inject it alongside the other preludes.

**Decision rationale.** Prefer **Option B** (a real prelude file) — it keeps
`Ground` as a normal discipline with no special-case code, and it's the
approach the spec implies ("predefined" = in the prelude, not "magic in the
compiler"). The codegen's ground-name detection (`gnd`/`GND`/`vss`/`VSS`) is
a *net-naming* convention, separate from the discipline type; both can
coexist.

**Verification.** Test that `mod R ( inout p : Electrical, inout n : Ground
) { ... }` elaborates and that `gnd : Ground` wires resolve. Add an
elaboration test in `crates/piperine-lang/tests/elaboration_tests.rs` that
exercises a resistor to `Ground`.

**Acceptance criteria.**
- [ ] `src/stdlib/ground.phdl` exists and is injected by the resolver.
- [ ] `gnd : Ground` elaborates.
- [ ] Existing examples (`delta_sigma.phdl`, `oscillator.phdl`) elaborate.
- [ ] Tests pass.

---

### C.2 Add `UInt[N]` and `SInt[N]` to the prelude as bundles

**Spec:** §6.6 — "`UInt[N]` and `SInt[N]` are bundles over `Bit[N]`
implementing the arithmetic capabilities in PHDL — letting vectors, buses,
and numeric types be defined rather than built in."

**Status:** MISSING. **High** (blocks `Accumulator`, `Driver[N]`, etc.).

**Current state.** `src/stdlib/` has only `capabilities.phdl` (19 lines) and
`collections.phdl` (11 lines). The example `tests/examples/capabilities.phdl`
defines `bundle UInt[N] { bits : Bit[N] }` but it's a user file, not prelude.
`SInt[N]` does not exist anywhere.

**Proposed solution.** Add `src/stdlib/integers.phdl`:

```phdl
discipline Bit { storage Boolean; }

bundle UInt[N] { bits : Bit[N] }

impl Add for UInt[N] {
    fn add(self, o: Self) -> Self {
        var r : Bit[N];
        var carry : Boolean = 0;
        for i in 0..N {
            r[i]  = self.bits[i] ^ o.bits[i] ^ carry;
            carry = (self.bits[i] & o.bits[i]) | (carry & (self.bits[i] ^ o.bits[i]));
        }
        return UInt[N] { .bits = r };
    }
}

// Sub, Mul, Div, Eq, Ord, BitAnd, BitOr, BitXor, Not similarly ...

bundle SInt[N] { bits : Bit[N] }

// signed variants — two's complement arithmetic
```

Inject via the resolver (`resolve/mod.rs:108-120`).

**Why this depends on other Parts.** This file uses generics (`UInt[N]` is a
const-param bundle, which works today; the `impl Add for UInt[N]` requires
`Self` handling (I.3) and capability dispatch (I.2) to actually *do*
anything at codegen). So the file can be added now (parse + register), but
the `impl` bodies only become executable after Part I.

**Decision rationale.** Adding the file now is cheap and unblocks examples
to *parse and elaborate* (with Part C.1 and the generic-bundle work in I.4).
The actual arithmetic execution is a codegen follow-up.

**Verification.** Test that `var x : UInt[8] = 0;` elaborates (after I.4).
For now, test that the prelude file parses and the bundles are registered.

**Acceptance criteria.**
- [ ] `src/stdlib/integers.phdl` exists and is injected.
- [ ] `UInt[N]` and `SInt[N]` registered after elaboration.
- [ ] Parse/elaborate test passes (execution test deferred to I.2/I.3).

---

### C.3 Root capabilities `Type` and `Net` are predefined

**Spec:** §6.6 — "`Type` (any value type) and `Net` (any net type) are the
root capabilities."

**Status:** MISSING. **Medium.**

**Current state.** The names `Type` and `Net` are not special-cased; they're
parsed as opaque identifier bounds.

**Proposed solution.** Add to `src/stdlib/capabilities.phdl`:

```phdl
capability Type { }
capability Net  { }
```

And in the typecheck/elaborator, treat `Type` as satisfied by any value type
and `Net` as satisfied by any net type (a check in the bound-validation pass
of I.5).

**Decision rationale.** Empty marker capabilities are the standard way to
express "any of a kind". The bound check consults the kind of the
substituted type.

**Verification.** Test that `bundle Pair <T: Type> { ... }` accepts `T =
Real` and rejects `T = Electrical`.

**Acceptance criteria.**
- [ ] `Type` and `Net` in prelude.
- [ ] Bound validation (I.5) treats them as kind-markers.
- [ ] Tests pass.

---

## Part D — Codegen: forces, analog operators, noise, functions

> The codegen today handles resistive `I(p,n) <+ …` contributions and `ddt`
> (via companion model). Everything else analog is fail-loud. This Part
> extends the codegen to cover the spec's analog surface.

### D.1 Potential forces `V(p,n) <- expr` (ideal voltage source)

**Spec:** §8.2 — "`<-` Force. Imposes a single-driver value — an ideal source
or short." Appendix A `VSource`, `OpAmp`, `BitToVoltage` all use `V(p,n) <-`.

**Status:** STUB fail-loud. **Critical** (blocks the spec's canonical
examples).

**Current state.** `ir_analog_to_device.rs:206-211`:

```rust
Stmt::Contrib { nature: IrNature::Potential(..), .. } => {
    return Err(CodegenError::Unsupported("potential contribution ..."));
}
```

**Why it matters.** Without `V(p,n) <-`, the spec's `VSource`, `OpAmp`,
`BitToVoltage`, `Dac` (Appendix B.1) cannot run. This is the single biggest
"the spec's examples don't work" gap.

**Proposed solution.** Implement potential forces as MNA voltage-source
branch-current rows. This is a solver extension — see H.4 for the MNA
machinery. At the codegen level:

1. `ir_analog_to_device` collects `Force { nature: Potential, plus, minus,
   expr }` into a separate `Vec<ForceContrib>` (not the `Contribution` list).
2. The JIT compiles a `force_residual(*const f64, *const SimCtx, *mut f64)`
   function that evaluates `expr` and writes `rhs[branch_idx] = V(plus) -
   V(minus) - expr`.
3. `JitAnalogDevice` exposes `eval_force` and the solver (H.4) stamps the
   branch-current row: an extra unknown `I_branch` with `V(plus) -
   V(minus) - expr = 0` and the branch current flowing into `plus`/out of
   `minus`.

**Decision rationale.** Ideal voltage sources are a fundamental MNA element.
The standard formulation adds one row per source: `V+ - V- = expr` with a
new branch-current unknown. The companion `expr` is evaluated by the same
JIT machinery as contributions (it's just an `IrExpr`).

**Verification.**
- Unit test: a `VSource` with `V(p,n) <- 1.0` produces an operating point
  where `V(p) - V(n) == 1.0`.
- E2E test in `tests/codegen_e2e_tests.rs`: `VSource(1V)` driving
  `Resistor(1kΩ)` to ground → `V(p) = 1V`, `I = 1mA`.
- Test `OpAmp` (B.5): `V(out) <- gain * V(inp, inn)` with a resistive
  feedback network produces the expected closed-loop gain.

**Acceptance criteria.**
- [ ] `Force { nature: Potential, .. }` collected and JIT-compiled.
- [ ] Solver stamps branch-current rows (H.4 done).
- [ ] `VSource`/`OpAmp`/`BitToVoltage` E2E tests pass numerically.
- [ ] `cargo test -p piperine-codegen -p piperine-solver` green.

---

### D.2 `idt`, `idtmod` integration operators

**Spec:** §8.1 — `idt(x)` integrates `x` w.r.t. time.

**Status:** STUB fail-loud. **High.**

**Current state.** `ir_analog_to_device.rs:105-110` rejects everything except
`Ddt`.

**Proposed solution.** `idt(x)` is the dual of `ddt`: state_next =
state_old + x * dt. Implement as a companion model:

- `idt` state slot holds the integral `∫x dt`.
- The charge-like stamp: `Q = state_old + x * dt` (state_old is updated each
  accept). The residual contribution is `I(p,n) <+ Q`.
- `idtmod(x, ic, modulus)` wraps the integral at `modulus`.

Implementation mirrors `build_reactive_contributions`
(`ir_analog_to_device.rs:88-125`) but for `Idt`/`IdtMod` kinds.

**Verification.** Test that `I(p,n) <+ idt(V(p,n))` produces a ramping
current under a constant voltage (an inductor-like behavior, since
`I = (1/L) ∫V dt`).

**Acceptance criteria.**
- [ ] `Idt` and `IdtMod` lower to companion stamps.
- [ ] Unit test passes numerically.
- [ ] No regression in `ddt` tests.

---

### D.3 `ddx`, `delay`, `transition`, `slew`, `laplace_*`, `zi_*`

**Spec:** §8.1, §8.2 — analog operators.

**Status:** STUB fail-loud. **Medium** (less common than `idt`).

**Proposed solution.** Implement incrementally, in this order:
1. `delay(x, t)` — ring buffer of past values (resistive stamp; reads delayed
   value).
2. `transition(x, td, tr, tf, tol)` — waveform queue with rise/fall times.
3. `slew(x, rise, fall)` — rate limiter.
4. `ddx(x, node)` — symbolic derivative w.r.t. a node voltage (computed at
   compile time via the existing `autodiff`).
5. `laplace_*` / `zi_*` — state-space filters; AC-only initially.

Each is a new `IrStateKind` arm in `build_reactive_contributions` (or
resistive for `delay`/`transition`/`slew`). See IR-SYSTEM §6 table for
integration/stamping per operator.

**Decision rationale.** Order by frequency of use in the spec examples.
`delay`/`transition` unblock digital-analog drivers; `laplace`/`zi` are
filter-specific.

**Verification.** One test per operator with a closed-form expected value.

**Acceptance criteria.**
- [ ] Each operator implemented with a unit test.
- [ ] No silent zero fallback for any `IrStateKind`.

---

### D.4 Noise sources are consumed by `Device::noise_current_psd`

**Spec:** §8 — noise sources (`white_noise`, `flicker_noise`) are part of the
analog surface; IR-SYSTEM §7 documents the IR.

**Status:** STUB. **Medium.**

**Current state.** IR captures noise sources faithfully
(`IrAnalogBody.noise_sources`, populated by both frontends), but
`ir_analog_to_device` never reads `body.noise_sources`, and
`PhdlDevice::noise_current_psd` returns `Vec::new()`
(`phdl_device.rs:238-244`).

**Proposed solution.**
1. `ir_analog_to_device` collects `body.noise_sources` into a `Vec<Noise>`
   on the `JitAnalogDevice`.
2. Each `IrNoiseSource { plus, minus, kind, label }` becomes a `Noise {
   terminals, value }` where `value` is a JIT-compiled PSD function (or a
   constant for `White { psd }` when `psd` is a literal).
3. `PhdlDevice::noise_current_psd` delegates to `self.analog.noise_sources`.

**Decision rationale.** The IR already has the data; the codegen just drops
it. The `Noise` solver struct (`piperine-solver`) is already consumed by
`NoiseSolver::solve` (`solver/noise.rs:94`), so once `PhdlDevice` emits
`Noise`s, the solver side works.

**Verification.** Test that a `noisy_resistor` PHDL model produces a
non-empty `noise_current_psd` and that the PSD value matches `4kT/R` within
tolerance (mirror the AMS OSDI test at `tests/osdi_integration.rs:329-661`).

**Acceptance criteria.**
- [ ] `JitAnalogDevice` carries `noise_sources: Vec<Noise>`.
- [ ] `PhdlDevice::noise_current_psd` non-empty for noisy models.
- [ ] Noise value test passes.
- [ ] `cargo test -p piperine-codegen -p piperine-solver` green.

---

### D.5 User `fn` calls are inlined at the call site

**Spec:** §7 — "Because it is pure it inlines at the call site, which is what
lets it serve every context uniformly."

**Status:** STUB fail-loud (analog) / silent 0 (digital). **High.**

**Current state.** `IrFunction` tables (`IrProgram.functions`,
`IrModule.functions`) are populated by both frontends but **read by no
codegen file** (only `display.rs` reads them for printing). Analog:
`validate_ir_contrib` rejects non-builtin calls (`ir_emit.rs:463-466`).
Digital: `Expr::Call(_, _) => DigitalVal::Natural(0)` silently
(`codegen/digital.rs:356-358`).

**Why it matters.** The spec's `thermal_voltage(t)` (Appendix A) and every
user-defined analog function is dead. `IR-SYSTEM.md:22` falsely claims the
codegen resolves user functions.

**Proposed solution.** Implement inlining in the codegen:

1. **At IR lowering** (`from_ppr.rs`/`from_ams.rs`), when an
   `IrExpr::Call(name, args)` is encountered and `name` is a known user `fn`
   (look up in `IrProgram.functions` and `IrModule.functions`), *inline* the
   function body: alpha-substitute the function's params with `args`, and
   replace the call with the body's `Return` expression. Do this recursively
   with a depth cap (e.g. 32) to prevent infinite inlining of (illegal)
   recursion.

   ```rust
   fn inline_user_call(
       prog: &IrProgram,
       module: &IrModule,
       e: &IrExpr,
       depth: u32,
   ) -> Result<IrExpr, String> {
       if depth > 32 { return Err("function inlining depth exceeded".into()); }
       match e {
           IrExpr::Call(name, args) if !is_builtin_math(name) => {
               let func = prog.functions.iter().chain(module.functions.iter())
                   .find(|f| f.name == *name)
                   .ok_or_else(|| format!("unknown function `{name}`"))?;
               let inlined_args: Vec<IrExpr> = args.iter()
                   .map(|a| inline_user_call(prog, module, a, depth+1)).collect::<Result<_,_>>()?;
               let mut subst = HashMap::new();
               for (p, a) in func.params.iter().zip(inlined_args.iter()) {
                   subst.insert(p.clone(), a.clone());
               }
               let body_expr = extract_return_expr(&func.body)
                   .ok_or_else(|| format!("function `{name}` has no return expr"))?;
               Ok(substitute(&body_expr, &subst))
           }
           // recurse into other variants, substituting
           other => recurse_substitute(other, |c| inline_user_call(prog, module, c, depth+1)),
       }
   }
   ```

2. Run this pass once on every analog contribution expression and every
   digital expression *before* `validate_ir_contrib`/`ir_digital_to_interp`.

3. **Purity check** (lightweight): a `fn` body must not contain `Contrib`/
   `Force`/`NonBlocking`/`Assign` to external nets. This can be a
   `validate_fn_purity` pass over `IrFunction` bodies. Start with a simple
   "no `<+`/`<-`/`=` to a non-local name" check.

**Decision rationale.** Inlining is the spec's stated semantic. It also
makes `fn` work uniformly in analog and digital (the spec's "serves every
context uniformly"). The depth cap is the backstop against the (illegal)
unbounded recursion the spec forbids (§7.1). Alternative — compiling `fn`s
as separate Cranelift functions — is more complex and offers no benefit for
small analog functions.

**Verification.**
- Test that `fn thermal_voltage(t: Real) -> Real { return 8.617e-5 * t; }`
  used in `I(a,c) <+ is_sat * (exp(V(a,c) / thermal_voltage(temp)) - 1.0);`
  inlines and produces the correct diode IV (same as the OSDI diode test,
  but through the PHDL/IR path).
- Test that a `fn` calling another `fn` inlines transitively.
- Test that a missing `fn` produces a clear "unknown function" error.
- Test that recursive `fn` (illegal) hits the depth cap with a clear error.

**Acceptance criteria.**
- [ ] `inline_user_call` pass implemented and run before validation.
- [ ] `IrFunction` table is read by codegen (no longer dead).
- [ ] Analog `fn` calls produce correct numerics.
- [ ] Digital `fn` calls produce correct values (no silent 0).
- [ ] Depth cap enforced.
- [ ] `IR-SYSTEM.md:22` updated to reflect actual behavior.
- [ ] Tests pass; `cargo test -p piperine-codegen` green.

---

### D.6 Digital interpreter: `if`/`match`/loops reachable from IR path

**Spec:** §8.3 — `if`/`else` and `match` are digital control flow.

**Status:** PARTIAL (works in interpreter, unreachable from IR). **High.**

**Current state.** The interpreter handles `If` and `Match`
(`codegen/digital.rs:291-311`), but `ir_digital_to_interp::lower_stmt`
(`ir_digital_to_interp.rs:116`) drops them with `_ => None`. So a digital
block with an `if` inside an `@` event compiles through IR but loses the
`if`.

**Why it matters.** Any non-trivial digital block (SAR ADC state machine,
synchronizer, etc.) is broken when lowered through the IR path.

**Proposed solution.** Extend `lower_stmt` (`ir_digital_to_interp.rs:54-118`)
to handle the missing `IrStmt` variants:

- `If { cond, then_, else_, .. }` → `ElabBehaviorStmt::If { cond: ir_expr_to_phdl(cond)?, then_body: lower_stmts(then_)?, else_body: lower_stmts(else_)? }`
- `Case { discriminant, arms, default, .. }` → `ElabBehaviorStmt::Match { ... }` (map each `(arm_expr, body)` to a `Pattern::Path` arm + wildcard default)
- `For { var, start, end, step, body }` — only if `start`/`end`/`step` are compile-time constants (unroll at lowering); otherwise reject.
- `While`/`Repeat`/`Forever` — reject (the spec §8.3 says unbounded loops are a compile error in digital; the spec actually forbids them only in `for` context but the principle applies).
- `VarDecl` → `ElabBehaviorStmt::VarDecl`
- `Return` — only valid inside a `fn` body, not a digital block; reject.

**Decision rationale.** The interpreter already has the eval machinery; the
gap is purely the IR→`ElabBehaviorStmt` translation. Mirroring `if`/`match`
is straightforward.

**Verification.** Test that a `digital` block with `if` inside `@posedge`
lowers and evaluates correctly. Use a DFF with a reset branch:

```phdl
digital DFF {
    q <- state;
    @ posedge(clk) {
        if (rst == 1) { state = 0; } else { state = d; }
    }
}
```

**Acceptance criteria.**
- [ ] `If`/`Case` lowered from IR to interpreter.
- [ ] `For` unrolled when bounds are const, rejected otherwise.
- [ ] `While`/`Repeat`/`Forever` rejected with clear error.
- [ ] DFF-with-reset test passes.

---

### D.7 Digital interpreter: `NonBlocking` vs `Assign` distinction

**Spec:** §8.3 — "`<-` drives a net; `=` assigns a `var`." A register
infers from a clocked `@` block.

**Status:** PARTIAL — both lowered to `Force` (`ir_digital_to_interp.rs:59-63`),
delay and event silently dropped.

**Current state.** The match arm `IrStmt::Assign { lval, expr, .. }` and
`IrStmt::NonBlocking { lval, expr, .. }` both produce
`ElabBehaviorStmt::Bind { op: Force }`, ignoring `delay` and `event`.

**Proposed solution.**
1. Preserve the distinction: `NonBlocking` → `Bind { op: NonBlocking }`,
   `Assign` → `Bind { op: Assign }`. The interpreter's `Bind` handling
   (`digital.rs:267-287`) should schedule a `DigitalEvent` for `NonBlocking`
   at the *next* delta cycle (or at `delay` time if present), and write
   immediately for `Assign`.
2. Preserve `event` timing control: an `Assign`/`NonBlocking` inside an
   `EventControl` is already wrapped in an `Event` stmt; a *top-level* event
   control on the assignment (e.g. `q <= d;` with no `@`) is combinational
   drive.

**Decision rationale.** The blocking/non-blocking distinction is the
Verilog semantic the spec inherits. Collapsing both to `Force` makes
pipelines collapse (the spec §8.3 explicitly says "a chain of register
writes is a pipeline, not a collapse").

**Verification.** Test a 2-stage shift register: `@ posedge(clk) { s1 <=
s2; s2 <= d; }` — `s1` must take the *old* `s2`, not the new one.

**Acceptance criteria.**
- [ ] `NonBlocking` scheduled at next delta; `Assign` immediate.
- [ ] Pipeline test passes (no collapse).
- [ ] `delay` field preserved and used.

---

### D.8 `$limit("pnjlim"|"fetlim", ...)` rejected; `limexp` is a stateless substitute

**Spec:** Verilog-A/AMS standard — `$limit("pnjlim", v, vold, vte, vcrit)`
applies SPICE's `DEVpnjlim` per-iteration voltage limiting; `$limit("fetlim", …)`
applies `DEVfetlim`. These are how SPICE guarantees quadratic convergence on
pn junctions and FET channels.

**Status:** STUB fail-loud + partial substitute. **High.**

**Cross-reference:** `docs/NGSPICE_GAPS.md` N.3, O.4; the `ngspice/`
headers (`diode.phdl:231`, `bjt.phdl:~600`, etc.) all use
`$limit("pnjlim", ...)` — fixing this gap removes the largest hard
deviation from NGSPICE.

**Current state.** The lowering produces
`SimQuery::Limit { kind: String, args: Vec<IrExpr> }` (`ir.rs:110-111`,
`lowering/expr.rs:462-469`). The JIT validator rejects it
(`ir_emit.rs:526-531`).

The `limexp` builtin is available as a *partial* substitute
(`codegen/cranelift_helpers.rs:43-47`):

```rust
"limexp" => {
    // Simplified: exp(min(u, 80))
    emit_call_exp(ctx, ...)
        // ...with the inner `u` clamped to ≤ 80 before exp ...
}
```

The comment says it is *"not stateful"* — it clamps the argument (preventing
`exp` overflow) but does **not** track the previous Newton iteration's
voltage, which is the core of `pnjlim`/`fetlim`. SPICE's `DEVpnjlim` is:

```
v_new = v_old + pnjl(v − v_old, vte, vcrit)
vstep = pnjl(v − v_old, vte, vcrit) − (v − v_old)
```

where `pnjl` limits the per-iteration step to a fraction of the critical
voltage `vcrit = n · vt · ln(n · vt / (√2 · is))`.

**Why it matters.** NGSPICE's iteration count on stiff diode circuits
(snap recovery, charge-pump startup) is governed by `pnjlim`/`fetlim`.
Without them:
- The simplified `limexp` prevents overflow (a hard error).
- But the Newton iteration can still take many extra steps or diverge, because
  the voltage update from one iteration to the next has no limiter.

The NGSPICE faithful models use:
- `diode.phdl:233` — `vd = $limit("pnjlim", ...)`
- `bjt.phdl:~600` — same for `vbe`, `vbc`
- `jfet.phdl`, `mos1.phdl` — `fetlim` for `vgs`, `vgd`

Replacing these with `exp(min(x, 80))` keeps the equations numerically safe
but loses the convergence property.

**Proposed solution.** Three parts:

1. **Stateful state slot:** add a new `IrStateKind::LimitSlot { site: u32 }`
   (mirrors `Ddt`/`Idt` state). Allocates a `f64` per `$limit` call site in
   the device's state vector. The slot stores the *previous Newton
   iteration's* value of the expression being limited.

2. **JIT emission:** in `emit_sim`:
   ```rust
   SimQuery::Limit { kind, args } => {
       let site = alloc_limit_slot(ctx);
       // vold := state[site];   // previous Newton value
       // v := evaluate(args[0]) using current voltages
       // vnew := apply_limit(kind, v, vold, args[1], args[2])
       // state[site] := v         // store for next Newton step
       // return vnew
   }
   ```
   where `apply_limit` dispatches `pnjlim`/`fetlim`/`limlog` based on
   `kind`.

3. **Codegen separation:** `limexp` stays the stateless
   `exp(min(x, 80))` for user code that just wants a clamped exponential
   (no Newton iteration storage cost). `$limit` is the Newton-aware
   primitive.

**Decision rationale.** Stateful state slots are the same mechanism `ddt`/
`idt` already use (`ir.rs:398-422`). Adding one more `IrStateKind` is
mechanical. The alternative — re-implementing pnjlim entirely on the solver
side — requires the solver to know about the expression's structure, which
couples the solver to the IR.

**Why this depends on other Parts.** The state-slot allocation machinery
must thread `state: &mut [f64]` through the Newton residual callback, the
same path D.1 (`<+`/`<-` contributions) uses. Once that exists, `LimitSlot`
is one more variant.

**Verification.**
- Iteration-count test: a diode reverse-snap circuit (PNP transistor
  pulling a charged node) starts with `vdrain = -5V` and a 10 mA
  current. With `pnjlim` it converges in ~5 iterations; without it,
  can take >50. The test asserts the iteration count.
- Functional test: a simple diode forward-bias — the DC IV curve with
  pnjlim matches NGSPICE's to ≤1% (the limit introduces ≤1% error on the
  bias voltage).
- Negative: `limexp(100.0)` returns `exp(80) ≈ 5.18e34`, not inf — confirms
  the stateless `limexp` still works.

**Acceptance criteria.**
- [ ] `SimQuery::Limit` accepted by `emit_sim`; `pnjlim` and `fetlim`
      implemented.
- [ ] Each `$limit` call allocates one state slot; slot is updated after
      each Newton iteration (not after each accepted step).
- [ ] Diode reverse-snap convergence test within 2× NGSPICE iteration count.
- [ ] `ngspice/headers/diode.phdl` compiles end-to-end through codegen
      (A.14 + A.15 + D.8 unblock all four semiconductor devices).
- [ ] `cargo test -p piperine-codegen -p piperine-solver` green.

---

### D.9 `ac_stim(mag, phase)` not implemented → AC analysis has no stimulus

**Spec:** Verilog-A/AMS standard — `ac_stim(mag, phase)` declares the
small-signal AC stimulus on a source; AC analysis uses it as the RHS
excitation.

**Status:** STUB. **Medium.**

**Cross-reference:** `docs/NGSPICE_GAPS.md` O.3.

**Current state.** `IrExpr::AcStim` exists in the IR `ir.rs` (line range
mentioned in the original GAPS audit) but is **validated out for analog
contrib** at `ir_emit.rs:568`. The PHDL lowering has no `$ac_stim`
syscall, so PHDL sources cannot declare an AC stimulus directly.

The faithful `vsrc`/`isrc` models in `ngspice/sources.phdl` use
`V(p, n) <+ ac_stim(ac_mag, ac_phase);` for the AC RHS. Today this is
a codegen-reject (silent zero, of the "no RHS excitation" variety).

**Why it matters.** Without `ac_stim`, every AC analysis of a PHDL model
returns the trivial solution (zero stimulus → zero response). The solver's
AC analysis infrastructure works (`solver/ac.rs`), but the source side has
nothing to inject. AC analysis is the diagnostic of choice for small-signal
filter design — losing it is losing half the value of mixed-signal
simulation.

**Proposed solution.** Two options, in order of preference.

**Option A (PHDL-idiomatic):** add `$ac_stim(mag, phase)` as a syscall
that lowers to `IrExpr::AcStim { mag, phase }`. The JIT emits:

```rust
// At AC analysis time only (AC-mode flag in SimCtx):
//   R[branch] += ac_mag * cos(ac_phase);
//   R[branch] += j * ac_mag * sin(ac_phase);
// At DC/tran time: emit nothing (or zero).
```

The `AnalysisKind` discriminator (D.10, below) decides which branch to
take.

**Option B (SPICE-style):** the solver reads the `AC` parameter from
source declarations out-of-band (not from the IR model body). The `vsrc`/
`isrc` types register with the solver at `TransientSolver::new`, the
solver reads `ac_mag`/`ac_phase` from the instance params, and injects
the stimulus during AC analysis independently of the device's residual.

**Decision rationale.** Adopt **Option A** for symmetry with the AMS
frontend (which already lowers `ac_stim` in its AMS lowering). Option B
is a fallback if the SimCtx plumbing proves too invasive. Both should
land before AC analysis is useful.

**Why this depends on other Parts.** D.10 (`$analysis`) is needed to
select the right branch in the JIT code (AC vs not-AC).

**Verification.**
- `vsrc` with `ac_mag=1.0` and `ac_phase=0.0` driving a `resistor(1k)`:
  AC analysis result is `|V/R| = 1e-3 mhos` at phase `0°`.
- Phase nonzero: `ac_phase=90°` → response phase is `90°` (imaginary).
- Negative: at DC operating point, `ac_stim` contributes nothing (no
  rhs offset, no bias shift).

**Acceptance criteria.**
- [ ] `$ac_stim(mag, phase)` syscall accepted by PHDL lowering.
- [ ] `IrExpr::AcStim` stamped into AC RHS only; no contribution to DC/tran
      residuals.
- [ ] AC magnitude + phase test (1 kHz, 10 kHz, 100 kHz) passes numerically.
- [ ] `cargo test -p piperine-codegen -p piperine-solver` green.

---

### D.10 `$analysis("kind")` rejected at codegen → no analysis-type branching

**Spec:** §8 — "Behavior may branch on the current analysis via `$analysis`,
which returns an `Analysis` enum (`Dc`, `Ac`, `Tran`, `Noise`); the
compiler specializes each analysis, so the branch costs nothing at runtime."

**Status:** STUB. **Medium.**

**Cross-reference:** `docs/NGSPICE_GAPS.md` N.4; also needed for D.9 above.

**Current state.** The lowering produces `SimQuery::Analysis(String)`
(`ir.rs:104-105`, `lowering/expr.rs:405-411, 478-486`). The validator
rejects every `SimQuery` other than `Temperature | Vt | Abstime |
Mfactor` at `ir_emit.rs:526-531`. `SimCtx` has **no** `current_analysis`
field (`codegen/mod.rs:56-82` — only `temperature`, `abstime`,
`mfactor`, `gmin`).

The NGSPICE faithful `vsrc`/`isrc` use `if ($analysis("tran")) { val =
waveform($abstime) } else { val = dc }` to switch between DC and transient
behavior. This lowers to `SimQuery::Analysis` and is currently a silent
reject.

**Why it matters.** Without `$analysis`:
- The `ddt` operator naturally evaluates to 0 in DC (its `StateRef`
  returns 0.0 in the resistive residual path per `ir_emit.rs:97-99`),
  which accidentally handles the *charge* side correctly.
- But the *source value* side does NOT work: a transient waveform is
  unconditionally computed during DC analysis, giving the wrong operating
  point. Conversely, a DC `dc = 1.0` value is computed during transient
  analysis if the user only sets `dc` (no waveform).

**Proposed solution.** Two parts.

1. **Add `current_analysis: AnalysisKind` to `SimCtx`** (alongside the
   existing fields), with the `#[repr(C)]` layout unchanged for
   existing fields:
   ```rust
   #[repr(C)]
   pub struct SimCtx {
       pub temperature: f64,      // offset 0
       pub abstime: f64,          // offset 8
       pub mfactor: f64,          // offset 16
       pub gmin: f64,              // offset 24
       pub current_analysis: u32, // offset 32 — Dc=0, Ac=1, Tran=2, Noise=3
       // (future: step, tfinal at offsets 40, 48 for A.14)
   }
   ```

2. **Solver sets it** at each call: `DcSystem::solve` → `0`,
   `AcSolver::solve` → `1`, `TransientSolver::step` → `2`,
   `NoiseSolver::solve` → `3`. One store per analysis (no per-iteration
   cost).

3. **JIT emission:**
   ```rust
   SimQuery::Analysis(kind) => match kind.as_str() {
       "dc"    => load_offset(ctx.sim_ctx, 32) == 0.0 ? 1.0 : 0.0,
       "tran"  => load_offset(ctx.sim_ctx, 32) == 2.0 ? 1.0 : 0.0,
       "ac"    => load_offset(ctx.sim_ctx, 32) == 1.0 ? 1.0 : 0.0,
       "noise" => load_offset(ctx.sim_ctx, 32) == 3.0 ? 1.0 : 0.0,
       _       => f64const(0.0),
   }
   ```

**Decision rationale.** This is the same `SimCtx`-threads-state pattern as
A.2/A.14. The string-based dispatch matches the spec's `$analysis("name")`
form, so no IR variant is needed.

**Verification.**
- `$analysis("dc")` returns 1 during DC, 0 during tran.
- `$analysis("tran")` returns 1 during tran, 0 during DC.
- DC operating point of `vsrc { .dc = 1.0 }` is `V = 1.0` (verified
  numerically).
- Transient of `vsrc { .dc = 1.0, .pulse_v1 = 0, .pulse_v2 = 2 }` with
  `wave = 2` (PULSE) gives `V = 0` at `t < td` (uses `dc`),
  `V = 2` at `td < t < td+pw` (uses waveform).

**Acceptance criteria.**
- [ ] `SimQuery::Analysis` accepted by `emit_sim`; `SimCtx.current_analysis`
      is written by solver at each analysis type.
- [ ] DC vs tran branching test (PULSE source) passes.
- [ ] `ngspice/headers/sources.phdl` compiles end-to-end through codegen
      (combined with A.14/A.15/D.8/D.9).
- [ ] `cargo test -p piperine-codegen` green.

---

## Part E — Mixed-signal bridges (A2D / D2A)

> The spec's §8 mixed-signal story ("a comparator is `digital`, a 1-bit DAC
> is `analog`") has **no implementation** today. `PhdlDevice` keeps `analog`
> and `digital` as independent sub-objects; the solver passes `av = &[]` to
> `eval_discrete`. This Part implements the bridge.

### E.1 Solver passes real analog voltages to `eval_discrete`

**Spec:** §8 — "A `digital` block ... may read digital values and sample
analog quantities." §8.4 — `cross`/`above` couples the domains.

**Status:** NOT IMPLEMENTED. **Critical** (blocks all mixed-signal examples).

**Current state.** `Device::eval_discrete(t, nets, analog_voltages, queue)`
has the `analog_voltages: &[f64]` parameter, but every call site passes
`&[]` (`crates/piperine-solver/src/topology.rs:158, 223`). The
`Device` trait comment at `src/device.rs:86-88` says "currently always
`&[]` but wired for future mixed-signal use".

**Why it matters.** A `Comparator` (`digital Comparator { out <- (V(vp) >
V(vn)); }`) cannot work through the solver loop — `V(vp)` reads 0 because
`av` is empty. Every mixed-signal example (SAR ADC, delta-sigma,
synchronizer) is blocked.

**Proposed solution.** Thread analog voltages into the digital evaluation:

1. The transient solver maintains the analog solution vector `x: Vec<f64>`
   (node voltages + branch currents). After each analog solve, build a
   `analog_voltages: Vec<f64>` indexed by *device terminal index*.
2. Each `Device` declares which analog terminals it reads (a new method
   `analog_input_terminals() -> &[AnalogReference]` on `Device`, default
   `&[]`). The solver collects these per device and builds a compact
   `analog_voltages` slice for each `eval_discrete` call.
3. `eval_discrete` receives the real `&[f64]`. The `PhdlDevice` passes it
   through to the `DigitalInterpreter`, which makes it available to
   `BranchAccess "V"` reads in digital expressions.

**Implementation sketch:**

```rust
// piperine-solver/src/device.rs — extend Device trait:
fn analog_input_terminals(&self) -> &[AnalogReference] { &[] }

// piperine-solver/src/solver/transient.rs — in the run_digital_at call:
let av_per_device: HashMap<usize, Vec<f64>> = build_analog_slices(
    &circuit.devices, &netlist, &x);
for (idx, dev) in circuit.devices.iter().enumerate() {
    let av = av_per_device.get(&idx).map(|v| v.as_slice()).unwrap_or(&[]);
    dev.eval_discrete(t, &digital_state.nets, av, &mut queue);
}
```

The `PhdlDevice` (`piperine-codegen/src/phdl_device.rs:268-278`) must
populate `analog_input_terminals` from the digital body's `BranchAccess`
reads (scan the digital body for `V(...)` accesses and map the terminal
names to `AnalogReference`s).

**Decision rationale.** This is the minimal change that unblocks mixed-signal.
The per-device slice avoids copying the whole solution vector. The
`analog_input_terminals` declaration makes the data flow explicit (no magic).

**Verification.**
- Test: a `Comparator` digital device driven by an analog ramp through the
  transient solver flips its output at the threshold. Mirror the existing
  `test_a2d_event_timing` (`tests/cosim_integration.rs:268-318`) but with
  the `Comparator` as a real `PhdlDevice`, not a test helper.
- Test: `BitToVoltage` analog device reads a digital net and forces `V(a)`
  to `vhigh`/`vlow`. This needs D.1 (forces) and E.2.

**Acceptance criteria.**
- [ ] `Device::analog_input_terminals` added.
- [ ] Transient solver builds and passes real `analog_voltages`.
- [ ] `PhdlDevice` populates `analog_input_terminals` from digital body.
- [ ] `Comparator` E2E test passes through the solver loop.
- [ ] `cargo test -p piperine-solver` green.

---

### E.2 D2A bridge: analog device reads digital state and stamps accordingly

**Spec:** §8 — "a 1-bit DAC is `analog` (reads a `Bit`, forces `V`)".
Appendix A `BitToVoltage`, Appendix B.8 `DeltaSigma` feedback.

**Status:** NOT IMPLEMENTED. **Critical.**

**Current state.** `PhdlDevice::load_dc`/`load_transient`
(`phdl_device.rs:198-236`) only consult `self.analog`. The digital state is
not visible to the analog stamping.

**Proposed solution.** Add a *digital-state read* path to the analog
loading:

1. `Device::load_dc` (and `load_transient`) gains access to the current
   `DigitalState` (or a per-device slice of relevant net values). Add a
   parameter `digital_state: &[LogicValue]` (or a typed wrapper) to
   `load_dc`/`load_transient`. Default `&[]` for pure-analog devices.
2. `PhdlDevice::load_dc` reads the digital nets it depends on (a new
   `digital_input_nets_for_analog()` method, derived by scanning the analog
   body for digital-net reads — e.g. `if (d == 1)` where `d` is a `Bit`
   port) and stamps a Thevenin source based on the digital value.
3. The `BitToVoltage` example:
   ```phdl
   analog BitToVoltage {
       var v : Real = if (d == 1) { vhigh } else { vlow };
       V(a) <- v;
   }
   ```
   lowers to: read `d` from `digital_state`, evaluate `v`, stamp
   `V(a) = v` (a force — D.1).

**Implementation sketch for `Device` trait:**

```rust
// piperine-solver/src/device.rs
fn load_dc(
    &mut self,
    netlist: &Netlist,
    ctx: &Context,
    digital_state: &[LogicValue],   // <-- NEW
) -> Vec<Stamp> { Vec::new() }
```

The solver passes the full `digital_state.nets` (or a per-device slice).
`PhdlDevice` maps digital net indices to its `BranchAccess`/comparison
reads.

**Decision rationale.** Symmetric to E.1: analog reads digital via an
explicit declared dependency. The Thevenin stamp (a force with finite
output resistance) is the spec's "ideal element defined by a pure
constraint is approximated with finite parameters" (§8.2).

**Verification.** `DeltaSigma` (Appendix B.8) is the canonical closed-loop
test: an analog integrator, a clocked 1-bit quantizer (digital), and a
feedback that reads the digital `q` into the analog block. Run a transient
and assert the modulator output bitstream has the correct DC value
(= average of `vin / vref`).

**Acceptance criteria.**
- [ ] `Device::load_dc`/`load_transient` take `digital_state`.
- [ ] `PhdlDevice` reads digital nets in analog stamping.
- [ ] `BitToVoltage` E2E test passes.
- [ ] `DeltaSigma` closed-loop transient test passes (bitstream average
      within tolerance of `vin/vref`).
- [ ] `cargo test` green.

---

### E.3 `cross`/`above` analog events drive digital state

**Spec:** §8.4 — "An analog crossing (`cross`/`above`) may drive digital
state, which is how a comparator or level detector couples the domains."

**Status:** PARTIAL (parsed, not driven by solver). **High.**

**Current state.** `cross`/`above` are parsed and validated
(`piperine-lang/src/elab/event.rs:33-41`) but no solver mechanism detects
the crossing and fires a digital event.

**Proposed solution.** Add an *analog-event detector* to the transient
solver:

1. Each `Device` with a `cross`/`above` event in its digital body declares
   the analog expression it watches (e.g. `cross(V(p,n))` watches
   `V(p,n)`). Add `Device::analog_event_probes() -> &[AnalogEventProbe]`
   where `AnalogEventProbe { expr_id, kind: Cross/Above, direction }`.
2. The transient solver, after each accepted step, evaluates each probe's
   expression at `t_prev` and `t_now`. If a crossing is detected (sign
   change of `expr` for `cross`, `expr` rises above 0 for `above`), the
   solver pushes a `DigitalEvent` at the crossing time (linearly
   interpolated) onto the event queue.
3. The device's `eval_discrete` then fires the `@ cross(...)` block when
   it sees the event.

**Decision rationale.** Analog-event detection is a solver responsibility
(it has the analog solution). The spec explicitly says this is how the
domains couple. The interpolation gives sub-step accuracy.

**Verification.** A `Comparator` built as `digital Comparator { @
cross(V(vp) - V(vn)) { out <- 1; } }` driven by a ramp — the event fires
at the exact crossing time.

**Acceptance criteria.**
- [ ] `Device::analog_event_probes` added.
- [ ] Transient solver detects crossings and pushes events.
- [ ] `cross`-based comparator test passes.
- [ ] `cargo test -p piperine-solver` green.

---

### E.4 `@ above` / `@ cross` events fire and update persistent `var` state (switch hysteresis)

**Spec:** §8.4 — "`above(expr)` fires when expression exceeds zero."
Implicit: the event body can mutate state (§5.2: `var` is stateful in
behavior blocks).

**Status:** PARTIAL (events parse but never fire). **Medium.**

**Cross-reference:** `docs/NGSPICE_GAPS.md` Q.3; `ngspice/switches.phdl`
declares `@ above(...) { sw_state = 1.0; }` in its `sw` and `csw` models.
Combines with I.15 (persistent `var`) — *both gaps block the NGSPICE
faithful switch models.*

**Current state.** The IR exposes the event mechanism (`IrEventKind::Above|
Cross|...Posedge|Initial`, `ir.rs:371-386`), but no solver code watches an
analog expression and fires the event. E.3 covers the *digital-output*
case (`@ cross(...) { out <- 1; }`); this entry covers the *state-update*
case (`@ above(...) { sw_state = 1.0; }`).

The difference: E.3's event is *observable* — when the event fires, the
device's digital output changes and other devices see it. E.4's event is
*internal* — it mutates the device's own state (used by the next
analog iteration). Same mechanism, different output.

**Why it matters.** The NGSPICE faithful `sw` and `csw` models are the
direct motivation:

```phdl
analog sw {
    var vc : Real = V(cp, cn);
    @ above(vc - (model.vt + model.vh))  { sw_state = 1.0; }
    @ above((model.vt - model.vh) - vc)  { sw_state = 0.0; }
    var g : Real = if (sw_state > 0.5) { gon } else { goff };
    I(p, n) <+ g * V(p, n);
}
```

Without E.4, `sw_state` stays at its initial value (`0.0`) and the
switch never transitions.

**Proposed solution.** Same event-detection mechanism as E.3, but the
target is a `var` slot (allocated per I.15), not a digital net. The
event handler writes to the slot; the analog body reads from it.

Internally, the difference from E.3 is the *sink* of the event: digital
net write vs. `var` slot write. The solver side reuses the same crossing
detector (timestamp interpolation, queue management).

**Decision rationale.** E.3's infrastructure carries over — the only
new code is the `var` write path (which I.15 already laid the foundation
for) and the event-handler dispatch (which the IR already represents).

**Why this depends on other Parts.** I.15 (persistent `var`) must land
first or sooner, since the event writes to a slot that must persist
across Newton iterations.

**Verification.**
- `sw` with positive hysteresis (`vt=2`, `vh=1`):
  - `vc(t) = 0.5 + t` (ramp from below vt): no event until `t = 1.5`
    (exceeds `vt+vh=3`).
  - At `t=1.5`: event fires, `sw_state = 1.0`.
  - `g = gon` for `t > 1.5`.
  - When ramp reverses and `vc < vt-vh=1`: event fires, `sw_state = 0`,
    `g = goff`.
- `csw` mirrors the above with current control (`I(cp,cn)`).
- NGSPICE faithful `switches.phdl` E2E: a relay circuit switching at
  the right thresholds.

**Acceptance criteria.**
- [ ] `@ above(expr)` writes to persistent `var` slot (I.15).
- [ ] Event firing does not require net write (no digital output).
- [ ] Switch hysteresis test matches expected transition times.
- [ ] NGSPICE `switches.phdl` E2E (combined with D.1, H.4, A.1, I.15).
- [ ] `cargo test -p piperine-codegen -p piperine-solver` green.

---

### E.5 `@ initial` fires once at t=0 for IC / off initialization

**Spec:** §8.4 — "`initial` / `final` | once, at t=0 / end." Implied:
the event body can set initial conditions via `V(p,n) <- ic;` forces
or `var = X;` writes.

**Status:** PARTIAL (recognized in IR, not fired by solver). **Medium.**

**Cross-reference:** `docs/NGSPICE_GAPS.md` Q.5; NGSPICE faithful
`cap.phdl:194`, `ind.phdl:255`, `diode.phdl:308`, `switches.phdl:50`
all use `@ initial { ... }` for state initialization.

**Current state.** `IrEventKind::Initial` exists in the IR
(`ir.rs:371-386`). The `digital_init` machinery at
`piperine-solver/src/circuit.rs:167-179` runs `initial` events for
*digital* devices (DFF reset, etc.) at t=0. There is no analogue for
*analog* devices — the `@ initial` body of a `cap`, `ind`, `dio`, or
`sw` never executes.

The NGSPICE faithful headers use `@ initial` for:
- `cap`: `@ initial { if (ic != 0.0) { V(p, n) <- ic; } }` — sets
  initial capacitor voltage for UIC transient.
- `ind`: `@ initial { if (ic != 0.0) { I(p, n) <- ic; } }` — initial
  inductor current.
- `dio`: `@ initial { if (ic != 0.0) { V(pp, n) <- ic; } }` — initial
  junction voltage.
- `sw` / `csw`: `@ initial { sw_state = if (off) { 0.0 } else { 1.0 }; }`
  — initial switch state for DC operating point.

**Why it matters.** Without `@ initial` firing:
- UIC transient simulations start from the solver's guessed values, not
  the user's `ic=` declarations.
- The DC operating point of switches with `off`/`on` flags ignores the
  user's intent (always starts open).

**Proposed solution.** Mirror the digital initial-event machinery for
analog:

1. Add `analog_init: Vec<IrStmt>` to `IrAnalogBody` (or reuse the existing
   `Event { spec: Initial, body }` representation already in the IR).
2. In the solver's pre-DCOp setup and pre-transient step-0 setup, execute
   each device's `analog_init` statements exactly once. The statements
   can include:
   - `V(p, n) <- expr;` — a force (D.1), sets initial node voltage.
   - `var_x = value;` — a `var` write (I.15), sets initial state.
3. The execution runs **before** the first Newton iteration, so the
   values are visible during the first residual evaluation.

```rust
// piperine-solver/src/circuit.rs (sketch):
fn init_analog(&mut self) {
    for (dev_idx, dev) in self.devices.iter_mut().enumerate() {
        for stmt in dev.analog_initial_events() {
            dev.exec_initial_stmt(stmt, &mut self.x, &mut self.state);
        }
    }
}
```

`exec_initial_stmt` interprets the IR statement with access to:
- `x: &mut [f64]` — node voltages / branch currents.
- `state: &mut [f64]` — `var` slots and reactive state (ddt/idt accumulators).
- A `SimCtx` (per A.2) for system queries.

**Decision rationale.** The mechanism mirrors the digital
`init_digital()` (H.5) — it's just another "fire-once-at-setup" event.
Adding `init_analog()` next to it is mechanical.

**Why this depends on other Parts.** D.1 (forces) for the `V <-` form,
I.15 (`var` persistence) for the `var =` form. Without those, only the
`var` form works initially; the `V <-` form unblocks once D.1 lands.

**Verification.**
- `cap` with `ic=1.0`: transient at `t=0+` reads `V ≈ 1.0` (with
  discretization error; exactly 1.0 with backward Euler at `t=0+`).
- `sw` with `off=1`: DC operating point converges with `sw_state = 0`
  (initial open). Open-circuit output.
- `sw` with `on=1` (uses param `off=0`): `sw_state = 1` at DC-OP. Closed.
- NGSPICE faithful `cap.phdl`/`ind.phdl`/`diode.phdl`/`switches.phdl`
  E2E: each device's IC is respected.

**Acceptance criteria.**
- [ ] `@ initial { V(p,n) <- ic; }` sets initial voltage (requires D.1).
- [ ] `@ initial { x = v; }` sets initial `var` value (requires I.15).
- [ ] Solver calls these once before DC-OP and once before t=0 transient.
- [ ] NGSPICE faithful headers E2E.
- [ ] `cargo test -p piperine-solver` green.

---

## Part F — `from_ir`: hierarchy, recursion, parent bodies

> `from_ir` (`crates/piperine-codegen/src/from_ir.rs`) assembles a
> `CircuitInstance` from an `IrProgram`. Today it only walks the *top*
> module's instances and never compiles the top's own analog/digital body.
> This Part makes it recursive and supports the spec's §5.3 (parent
> contributing to child terminals).

### F.1 `from_ir` recurses into child-module instances

**Spec:** §5 — a module may instantiate child modules, which may themselves
instantiate children.

**Status:** MISSING. **High.**

**Current state.** `from_ir.rs:81` loops only over `top_module.instances`.
A child module's own instances are never expanded.

**Proposed solution.** Recurse. For each instance, after compiling its
analog/digital body, also walk *its* `instances` and instantiate them
(relative to the parent's node namespace). This requires hierarchical net
naming: `parent.child.port`.

```rust
fn instantiate_module(
    prog: &IrProgram,
    module_name: &str,
    parent_prefix: &str,        // hierarchical path prefix
    port_net_map: &HashMap<String, NodeIdentifier>,  // port name → parent net
    out: &mut Vec<Box<dyn Device>>,
) -> Result<(), String> {
    let module = prog.modules.iter().find(|m| m.name == module_name)
        .ok_or_else(|| format!("unknown module `{module_name}`"))?;

    // 1. Compile this module's analog/digital body (if any) — see F.2.
    // 2. For each instance in module.instances, recurse with prefix
    //    `{parent_prefix}.{inst.label}` and a port_net_map built from
    //    inst.connections (resolved against this module's nets).
    for inst in &module.instances {
        let child_prefix = format!("{parent_prefix}.{}", inst.label);
        let child_port_map = resolve_connections(inst, module, parent_prefix)?;
        instantiate_module(prog, &inst.module, &child_prefix, &child_port_map, out)?;
    }
    Ok(())
}
```

**Decision rationale.** Hierarchical instantiation is standard SPICE/netlist
behavior. The prefix gives unique names; the port_net_map threads the
parent's nets into the child's port names.

**Verification.** Test a 2-level hierarchy: `Top` instantiates `Mid`
instantiates `Resistor`. The resulting `CircuitInstance` has 3 devices (Top
body, Mid body, Resistor) or 2 if Top/Mid are pure containers.

**Acceptance criteria.**
- [ ] `from_ir` recurses into child instances.
- [ ] Hierarchical names unique.
- [ ] Multi-level test passes.

---

### F.2 `from_ir` compiles the top module's own analog/digital body

**Spec:** §5.3 — "the parent may connect, probe, or contribute to from its
own `analog` block ... I(load.p, gnd) <+ cpar * ddt(V(load.p, gnd));"

**Status:** STUB. **High.**

**Current state.** `from_ir.rs:81` walks `top_module.instances` but never
reads `top_module.analog` or `top_module.digital`. The parent's own
contributions are lost.

**Why it matters.** The spec's `Ladder` (B.10), `SarAdc` analog block
(parasitic load on DAC node), and any "container with parasitics" pattern
is blocked.

**Proposed solution.**
1. After walking instances, compile `top_module.analog` (if present) via
   `ir_analog_to_device` and add the resulting `JitAnalogDevice` to the
   device list. Its terminals are the top module's *ports and wires*
   (including child-port refs like `load.p` which flatten to `load_p`).
2. Same for `top_module.digital`.
3. The `name.port` references in the parent's analog body must resolve to
   the child's port net — this is the hierarchical-net-name resolution of
   F.3.

**Decision rationale.** The parent is itself a device with analog/digital
behavior. Today `from_ir` treats it as a pure netlist container, which is
wrong per the spec.

**Verification.** Test the spec's `Ladder` (B.10): a `for` of named
resistor legs, with the parent's analog block adding a parasitic cap at
each `rseg[i].n`. The transient result must show the parasitic loading.

**Acceptance criteria.**
- [ ] Top module's analog/digital body compiled and added to device list.
- [ ] `Ladder` E2E test passes.
- [ ] `cargo test -p piperine-codegen` green.

---

### F.3 Hierarchical `name.port` and `name[i].port` references

**Spec:** §5.3 — "A named instance exposes each of its ports as a net
`name.port` ... An instance in a `for` is named as an array `name[i]`, and
`name[i].port` reaches the node of each replica."

**Status:** MISSING. **High.**

**Current state.** `from_ir.rs:103` treats connection `net` as a flat
string. `piperine-lang`'s `eval_net_ref` only reduces `Ident.field`
(`lower.rs:267-278`); `Field(Index(...), ...)` is not reducible
(`lower.rs:279-283`).

**Proposed solution.**
1. In `piperine-lang/elab/lower.rs`, extend `eval_net_ref` to handle
   `Expr::Field(Expr::Index(base, idx), field)`:
   ```rust
   Expr::Field(base, field) => match base.as_ref() {
       Expr::Ident(n) => Ok(ElabNetRef::simple(format!("{n}_{field}"))),
       Expr::Index(inner, idx) => {
           let base_name = expect_ident(inner)?;
           let i = env.eval_nat(idx)?;
           Ok(ElabNetRef::simple(format!("{base_name}_{i}_{field}")))
       }
       other => Err(NotANetRef(...))
   }
   ```
   This produces flat names like `rseg_0_n`, `rseg_1_n`, ... which the
   `for`-unrolled instances already use.
2. In `from_ir.rs`, parse `name.port` and `name[i].port` strings (or
   better: have the IR carry structured net refs instead of strings — see
   K.4).

**Decision rationale.** The flat-name convention (`{base}_{i}_{field}`)
matches what `for`-unrolling already produces (`rseg_0`, `rseg_1`, ...).
Extending `eval_net_ref` to handle the indexed-field form makes the
elaboration consistent.

**Verification.** The `Ladder` test (F.2) exercises `rseg[i].n`. Add a
unit test in `tests/elaboration_tests.rs` that `I(rseg[0].n, gnd)` resolves
to net `rseg_0_n`.

**Acceptance criteria.**
- [ ] `Expr::Field(Expr::Index(..), ..)` reduces in `eval_net_ref`.
- [ ] `from_ir` resolves hierarchical names.
- [ ] `Ladder` E2E test passes.

---

### F.4 Structural `for`/`if` (generate) in `from_ir`

**Spec:** §5.4, §5.5 — structural `for` over a constant range; structural
`if` with elaboration-constant condition.

**Status:** MISSING at IR level. **Medium** (PHDL elaboration unrolls them,
but AMS generate is dropped — see G.3).

**Current state.** `IrStmt::For` is a *runtime* loop, not a structural
generate. There is no `generate` IR node. PHDL's structural `for` is
unrolled at elaboration (`lower.rs:403-420`), so it does not appear in IR.
AMS `generate`/`loop_generate`/`if_generate`/`case_generate` is dropped at
`piperine-ams/src/parser.rs:362-366`.

**Proposed solution.**
- **PHDL:** no change needed (elaboration unrolls). Verify with tests.
- **AMS:** implement `loop_generate`/`if_generate` unrolling in
  `convert_module` (`piperine-ams/src/parser.rs`) before lowering to IR.
  A `generate` block with a constant `for` loop is unrolled by evaluating
  the loop bounds (must be constant expressions) and emitting one
  `ModuleInstantiation` per iteration.

**Decision rationale.** Generate is an elaboration concept; the IR is
right to not carry it. AMS just needs to unroll at the AST→Module step.

**Verification.** Test an AMS `generate for` produces the right number of
instances in the IR.

**Acceptance criteria.**
- [ ] AMS `generate for`/`if` unrolled into instances.
- [ ] Test passes; `cargo test -p piperine-ams` green.

---

## Part G — AMS frontend gaps

### G.1 AMS digital `initial`/`always` lowered to `IrDigitalBody`

**Spec:** N/A (AMS standard), but needed for mixed-signal AMS netlists.

**Status:** STUB. **High.**

**Current state.** `piperine-ams/src/parser.rs:362-366` drops
`InitialConstruct`/`AlwaysConstruct`. `from_ams.rs:210` hardcodes
`digital: None`. No AMS module ever produces an `IrDigitalBody`
(IR-SYSTEM.md:685 acknowledges this).

**Why it matters.** The AMS fixtures `dff.v`, `a2d.vams`, `d2a.vams`,
`testbench.v`, `clock_gen.v` (in `tests/fixtures_fmt/`) are pure-digital
Verilog that parse but never reach the digital interpreter.

**Proposed solution.**
1. In `piperine-ams::Module` (`model.rs:30-48`), add a field
   `digital_blocks: Vec<DigitalBlock>` (or reuse `AnalogBlock` shape with a
   flag).
2. In `convert_module` (`parser.rs:140-371`), handle
   `ModuleItem::InitialConstruct` and `ModuleItem::AlwaysConstruct` by
   converting their statements (the existing `convert_stmt` machinery
   handles `NonBlockingAssign`, `EventControl`, `If`, etc.).
3. In `from_ams.rs::convert_module`, populate `digital: Some(IrDigitalBody {
   ... })` from the converted blocks instead of `None`.

**Decision rationale.** The AMS statement-lowering already handles digital
flavors inside analog blocks (`from_ams.rs:426-488`); the gap is purely that
the *top-level* `initial`/`always` items are discarded before lowering. The
fix is plumbing, not new lowering logic.

**Verification.** Test that `dff.v` produces an `IrDigitalBody` with a
`NonBlocking` and an `EventControl(posedge(clk))`, and that
`ir_digital_to_interp` on it produces a working DFF.

**Acceptance criteria.**
- [ ] `Module.digital_blocks` populated.
- [ ] `from_ams` produces `IrDigitalBody` for AMS digital blocks.
- [ ] `dff.v` E2E test passes through `ir_digital_to_interp`.
- [ ] `cargo test -p piperine-ams -p piperine-codegen` green.

---

### G.2 AMS `param_ports` (header `#(parameter ...)`)

**Spec:** N/A (AMS standard).

**Status:** DROPPED. **Medium.**

**Current state.** `piperine-ams/src/grammar/item.rs:101-112` parses the
`#(parameter real x = 1.0, ...)` header into `ModuleDecl.param_ports`, but
`convert_module` (`parser.rs:140-159`) never reads `decl.param_ports`. They
survive only if re-declared in the body.

**Proposed solution.** In `convert_module`, after parsing the header, merge
`decl.param_ports` into the module's `parameters` list (body declarations
override header ones by name).

**Verification.** Test `module amp #(parameter real gain = 10.0) (in, out);`
without a body `parameter` decl — `gain` must appear in the IR params.

**Acceptance criteria.**
- [ ] `param_ports` merged into `Module.parameters`.
- [ ] Test passes.

---

### G.3 AMS `Parameter.constraints` (`from`/`exclude`)

**Spec:** N/A.

**Status:** DROPPED. **Low.**

**Current state.** `from_ams.rs:71-78` only reads `name`/`ty`/`default_value`;
`Parameter.constraints` (parsed at `parser.rs:125-138`) is dropped.

**Proposed solution.** Carry `constraints` into `IrParam` (add a field) and
have `from_ir::eval_ir_const` validate the resolved value against the
constraint at instantiation. A violation returns
`Err("param `{name}` value {v} violates constraint {constraint}")`.

**Decision rationale.** Param constraints are a validation feature; dropping
them silently means invalid parameter values pass through.

**Acceptance criteria.**
- [ ] `IrParam.constraints` added.
- [ ] `from_ir` validates param values against constraints.
- [ ] Test passes.

---

### G.4 AMS formatter test coverage

**Spec:** N/A.

**Status:** PARTIAL (no tests). **Low.**

**Current state.** `piperine-ams/src/fmt.rs` is a functional token
pretty-printer wired to `piperine-cli fmt`, but has **zero unit tests**, no
idempotency check, no golden snapshot. `tests/fixtures_fmt/` is a parse
corpus, not a formatter corpus (the name is misleading).

**Proposed solution.**
1. Add `tests/fmt_tests.rs` with: (a) idempotency — `format(format(input))
   == format(input)` for a dozen inputs; (b) a few golden snapshots.
2. Rename `tests/fixtures_fmt/` to `tests/fixtures_parse/` (or add a
   comment in AGENTS.md clarifying the name) — but per AGENTS.md these are
   frozen, so renaming is a path-change; prefer adding a README in the
   directory.

**Decision rationale.** A formatter without tests rots. Idempotency is the
minimal invariant.

**Acceptance criteria.**
- [ ] `tests/fmt_tests.rs` with idempotency + snapshot tests.
- [ ] `cargo test -p piperine-ams` green.

---

## Part H — Solver: integration, timestep, convergence

### H.1 Wire up trapezoidal integration (vs only backward Euler today)

**Spec:** IR-SYSTEM §6 — "Backward Euler / Trapezoidal".

**Status:** PARTIAL. **Medium.**

**Current state.** Only backward Euler (`alpha = 1/dt`,
`src/solver/transient.rs:153`). The `IntegrationMethod` enum
(`src/analysis/truncation.rs:39-57`) is defined with Trapezoidal + Gear
variants and correct coefficients but **never consulted**.

**Proposed solution.**
1. Add `IntegrationMethod` to `TransientAnalysisOptions` (default
   `BackwardEuler` for stability).
2. In `TransientSystem::assemble`, compute the companion coefficients based
   on the method:
   - BE: `alpha = 1/dt`, `beta = 0` (no history term).
   - Trap: `alpha = 2/dt`, `beta = 1` (history term `−x_prev + (2/dt − ...)`
     — the trapezoidal companion has a `x_prev` RHS contribution).
3. Devices' `load_transient` must apply `alpha` to reactive Jacobian and
   `beta`/history to RHS. Today `OsdiDevice::load_transient`
   (`osdi/device.rs:583, 602`) applies `alpha` only — extend it (and
   `PhdlDevice`) for `beta`.

**Decision rationale.** Trapezoidal is 2nd-order accurate vs BE's 1st-order;
standard SPICE offers both. Default BE for stability (the spec's examples
don't demand Trap). User-selectable.

**Verification.** Test a capacitor RC transient with both methods; trapezoidal
should be more accurate at the same `dt` (compare to closed-form
`V(t) = V0(1 − e^{−t/RC})`).

**Acceptance criteria.**
- [ ] `IntegrationMethod` selectable.
- [ ] Trapezoidal companion stamps correct.
- [ ] Accuracy test passes.
- [ ] `cargo test -p piperine-solver` green.

---

### H.2 LTE-based timestep control (the dead `TruncationError`/`BreakpointProvider` traits)

**Spec:** N/A (internal), but the spec §8.2 mentions `$bound_step(dt)`.

**Status:** STUB (infrastructure present, never called). **Medium.**

**Current state.** `TruncationError` and `BreakpointProvider` traits
(`src/analysis/truncation.rs:108-156`) are defined with unit tests for the
coefficient table but have **zero call sites**. `Context.trtol`/`chgtol`
dead. `Device::bound_step_hint` implemented by `OsdiDevice` but never called.
`Device::accept_timestep` implemented (maintains `charge_history`) but never
called, so `charge_history` is always empty.

**Proposed solution.**
1. In `TransientSolver::solve`, after a successful step, call
   `accept_timestep` on each device (populates `charge_history`).
2. Compute LTE per device: `lte = |Q_next − Q_pred|` where `Q_pred` is the
   polynomial-extrapolated charge from `charge_history`. Use
   `TruncationError::truncation_error` with the selected `IntegrationMethod`.
3. If `lte > chgtol * max(|Q_next|, |Q_prev|) + chgtol_abs` → halve `dt` and
   retry. Else accept and grow `dt` (capped by `max_step` and
   `bound_step_hint`).
4. Honour `$bound_step(dt)` — the IR `BoundStep(IrExpr)` stmt
   (`ir.rs:386`) carries a user-requested cap; the `PhdlDevice` exposes it
   via `bound_step_hint`.

**Decision rationale.** The infrastructure is already written; this is
wiring. LTE control is what makes transient simulators fast (large `dt` on
smooth regions, small `dt` on transitions).

**Verification.** Test that a capacitor charging with LTE control uses
larger `dt` on the flat tail and smaller `dt` on the rising edge, and that
the result matches the closed-form within `trtol`.

**Acceptance criteria.**
- [ ] `accept_timestep` called after each step.
- [ ] LTE computed and used for step control.
- [ ] `$bound_step` honoured.
- [ ] `TruncationError`/`BreakpointProvider` no longer dead.
- [ ] Test passes.

---

### H.3 gmin stepping and source stepping for DC convergence

**Spec:** §8.2 — "An ideal element defined by a pure constraint is
approximated with finite parameters (a large but finite gain), keeping every
statement a direct stamp."

**Status:** MISSING. **Medium** (blocks hard-nonlinear DC).

**Current state.** `Context.gmin` (default 1e-12 S) is **only forwarded to
OSDI plugins** (`osdi/ffi.rs:69-70`); the solver never adds gmin
conductances to the matrix. No gmin stepping, no source stepping. `min_res`
(`solver/mod.rs:24`) is dead.

**Why it matters.** Circuits with floating nodes (no DC path to ground)
fail to converge with no diagnostic. Hard-nonlinear DC (snapping PN
junctions) has no homotopy recovery.

**Proposed solution.**
1. **gmin stamping:** in `DcSystem::assemble`, add a `gmin` conductance from
   every node to ground (a `Stamp::Matrix(node, node, gmin)` per node).
   This regularises the matrix.
2. **gmin stepping homotopy:** start with `gmin = 1e-3` (large), solve, then
   reduce `gmin` by 10× each converged step until `gmin = 1e-12`. Each step
   uses the previous as initial guess.
3. **Source stepping:** start with sources at 0 (trivial DC), ramp to full
   value over steps, each step seeded by the previous.

**Decision rationale.** Standard SPICE convergence aids. gmin stepping is
the cheapest and unblocks floating nodes. Source stepping handles
hard-nonlinear cases.

**Verification.** Test a circuit with a floating node (e.g. an open-circuit
capacitor) converges with gmin stepping and reports the gmin value used.

**Acceptance criteria.**
- [ ] gmin stamped to ground per node.
- [ ] gmin stepping homotopy implemented.
- [ ] Floating-node test converges.
- [ ] `cargo test -p piperine-solver` green.

---

### H.4 Voltage-source branch-current rows in MNA

**Spec:** §8.2 — `V(p,n) <- expr` (force) is an ideal voltage source.

**Status:** MISSING. **Critical** (prerequisite for D.1).

**Current state.** The MNA matrix is `netlist.max_index() + 1` per node
only (`solver/dc.rs:114`). There is no branch-current unknown for voltage
sources. The OSDI tests deliberately avoid voltage sources
(`tests/osdi_integration.rs:7-9`).

**Proposed solution.**
1. Extend `Netlist` (`src/analog.rs:161-235`) to allocate a
   branch-current index for each declared voltage source (force). The index
   space is `node_indices + branch_indices`.
2. The MNA matrix dimension becomes `max_node_index + 1 + num_branches`.
3. For each voltage source: add a row `V+ − V− − expr = 0` (the constitutive
   equation) and stamp the branch current into the KCL rows of `V+` and
   `V−` (positive into `+`, negative into `−`).
4. The `force_residual` JIT function (D.1) evaluates `expr` and writes the
   row RHS; the Jacobian of `expr` w.r.t. node voltages is stamped into the
   row's node columns.

**Implementation sketch:**

```rust
// src/analog.rs — extend Netlist:
pub fn add_voltage_source(&mut self, plus: AnalogReference, minus: AnalogReference) -> usize {
    let idx = self.branch_counter.fetch_add(1, ...);
    self.branches.push(Branch { plus, minus, idx });
    idx
}

// src/solver/dc.rs — matrix dim:
let n = netlist.max_index() + 1 + netlist.num_branches();
```

**Decision rationale.** Standard MNA. The branch-current unknown is the
price of an ideal voltage source. This unblocks D.1 (`VSource`, `OpAmp`,
`BitToVoltage`) and A.1 (current reads via branch-current unknowns).

**Verification.** Test `VSource(1V)` into `Resistor(1kΩ)` → `V = 1V`,
`I_branch = 1mA`. Test the existing OSDI `vsource.va` fixture now works
(it's in the corpus but unexercised — `tests/va/vsource.va`).

**Acceptance criteria.**
- [ ] `Netlist` allocates branch-current indices.
- [ ] MNA matrix dimension includes branches.
- [ ] `VSource` E2E test passes numerically.
- [ ] OSDI `vsource.va` test enabled (was avoided).
- [ ] `cargo test -p piperine-solver` green.

---

### H.5 `init_digital()` called by the transient solver

**Spec:** N/A (internal).

**Status:** BUG. **Medium.**

**Current state.** `CircuitInstance::init_digital` (`src/circuit.rs:167-179`)
collects `digital_init` events and runs t=0 propagation, but
`TransientSolver::new` never calls it (`src/solver/transient.rs:105` only
calls `rebuild_digital_topology`). Tests work around this by manually
populating `digital_state`.

**Proposed solution.** Call `init_digital()` in `TransientSolver::new`
after topology build.

**Verification.** A digital device with `digital_init` events (e.g. a DFF
with reset) starts in the right state without manual test setup.

**Acceptance criteria.**
- [ ] `init_digital()` called in `TransientSolver::new`.
- [ ] Test passes; existing tests simplified (remove manual setup).

---

### H.6 BJT excess phase (PTF) needs a state-recurrence operator

**Spec:** Not in PHDL spec directly; this is a compact-model semantic that
surfaces as a numerical fidelity gap when porting the NGSPICE `bjt` model.

**Status:** STUB (model falls back to no excess phase). **Low.**

**Cross-reference:** `docs/NGSPICE_GAPS.md` Q.1.

**Current state.** The NGSPICE faithful `bjt.phdl` computes the BJT
collector current. NGSPICE's full implementation includes a "phase"
correction (`bjtload.c:498-519`): the transport current `cbe` is
filtered through a second-order Weil approximation that gives a frequency-
dependent phase delay matching the device's transit time behavior. The
NGSPICE implementation uses:

```c
td = excessPhaseFactor = (PTF / (180/π)) * TF  // seconds
cex = cbe filtered through H(s) = 1 / (1 + td/3·s + (td/3)²·s²)
```

PHDL's BJT model uses `cex = cbe` (no phase correction). When `model.ptf =
0` (the default), this gives exact parity. When `PTF > 0`, the model's
high-frequency accuracy degrades — AC analysis of an RF BJT at GHz
frequencies underestimates phase shift.

The NGSPICE `pjrc` BJT fixtures in `tests/fixtures/vams/` are at audio
rates, so the gap is invisible in the current corpus.

**Why it matters.** Faithful port of RF BJT behavior. Without PTF, RF
designers using the Piperine BJT model see wrong phase at
`f · TF > 0.1`.

**Proposed solution.** Three options:

1. **`delay(x, t)` operator** (per D.3) + chain. A second-order Weil
   filter is `H(s) = 1 / (1 + s·τ/3 + (s·τ/3)²)`. In the Laplace domain,
   this is two integrators and two additions. With `idt` operator (D.2)
   available, this becomes:
   ```phdl
   // Inside the BJT analog block when PTF > 0:
   var td : Real = (model.ptf * M_PI / 180.0) * model.tf;
   // two-idt chain forming the 2nd-order section:
   var stage1 : Real = idt(cbe);  // first pole at s = -3/τ
   var cex : Real = idt(stage1 - (3.0/td) * stage1 + (9.0/(td*td)) * idt(stage1));
   ```
   (Simplified; the actual circuit is an RC L-section.)

2. **Dedicated `state_recurrence(stmts)` operator.** A PHDL block that
   defines a discrete-time recurrence with explicit state. Mirrors what
   NGSPICE does in C. More flexible but adds a new IR kind.

3. **Skip for now** (current state). Document as a known accuracy gap.

**Decision rationale.** Adopt **Option 1** once `delay`/`idt` are
implemented (D.2/D.3). Option 2 is too language-invasive for one model's
quirk. The Weil approximation via two `idt` integrators is faithful to
the numerical behavior even if not bit-identical to the C code.

**Verification.**
- BJT with `PTF=30`, `TF=10e-12`: AC phase at `f=10GHz` matches
  `arg(H(jω))` within 5°.
- BJT with `PTF=0`: identical to current model.
- The `bjt.phdl` header gains a conditional block:
  ```phdl
  var cex : Real = if (model.ptf > 0.0 && model.tf > 0.0) { ... } else { cbe };
  ```

**Acceptance criteria.**
- [ ] PTF=0 → identical results to before.
- [ ] PTF=30, f=10GHz → phase within 5° of NGSPICE.
- [ ] No regression in `bjt.phdl` E2E tests with `model.ptf = 0` (default).
- [ ] `cargo test -p piperine-codegen -p piperine-solver` green.

---

### H.7 `const_eval` cannot iterate fixed-point for diode breakdown voltage

**Spec:** §7.1 — "Recursion is resolved entirely at elaboration and must
terminate." The spec accepts bounded recursion; it doesn't say iteration.
Iterative schemes (fixed-point) are a numerical necessity for the diode
breakdown model.

**Status:** STUB (first-order approximation used). **Low.**

**Cross-reference:** `docs/NGSPICE_GAPS.md` Q.4.

**Current state.** The NGSPICE faithful `diode.phdl` computes the
breakdown voltage temperature adjustment. NGSPICE's full algorithm
(`diotemp.c:208-244`) is a 25-iteration fixed-point:

```c
// First guess:
xbv = tBv - nbv * vt * log(1 + cbv / tSatCur);
// Iterate until |xcbv - cbv| < reltol * cbv, max 25 iterations:
for (iter = 0; iter < 25; iter++) {
    xbv = tBv - nbv * vt * log(cbv / tSatCur + 1 - xbv/vt);
    xcbv = tSatCur * (exp((tBv - xbv) / (nbv * vt)) - 1 + xbv / vt);
    if (fabs(xcbv - cbv) <= tol) goto matched;
}
```

The NGSPICE faithful header uses **only the first guess** (the pre-iteration
formula on line above). This is typically within ~1% of the iterated value,
which is acceptable for most circuits but not bit-identical.

`const_eval` (used for elaboration-time constants) does NOT currently
support `for` loops over runtime-controlled bounds. The breakdown voltage
is computed *at analog evaluation time* (since `T` and per-instance
parameters are not all elaboration constants), so it happens in the
analog block, not in `const_eval`. The constraint is that the analog block
cannot contain `for` (per spec §8: "A `for` in either block is unrolled
into hardware").

**Why it matters.** For circuits near breakdown (avalanche photodiodes,
Zener regulators, ESD clamps), the iterative refinement can matter at
the 1% level. Faithful RF/ESD simulation cares.

**Proposed solution.**

1. **In the analog block** (where the computation happens), introduce a
   bounded iteration via a fixed-trip expression. PHDL doesn't support
   `while` in analog blocks (per spec), so the iteration must be unrolled
   to a *bounded* `for` with const bound.

2. **Const-for in `const_eval`:** add `for i in 0..N` to const-eval,
   where `N` is a compile-time constant. This supports the recurrence
   being computed once per temperature, not per evaluation.

3. **Approximation fallback** (current state, document): keep the
   first-order formula; note in the BJT/diode accuracy section that the
   temperature-adjusted breakdown voltage is approximate.

**Decision rationale.** **Adopt option 3** (first-order approximation)
documented as a known accuracy gap. Full fixed-point requires either:
- A spec extension (analog `while`/`repeat`), or
- A dedicated operator.

Neither is worth the language complexity for a model corner-case. The
1% error is acceptable for the vast majority of circuits.

**Verification.**
- Test: `dio` with `bv=5.6`, `ibv=1mA`, `t=300K`: iteration-up-to-25 vs
  first-order — error within 1% on `tBrkdwnV`.
- Test: `dio` reverse sweep near `bv` — IV curve smooth, no
  convergence failures.
- `cargo test -p piperine-codegen` green.

**Acceptance criteria.**
- [ ] Documented accuracy gap; marked Low severity.
- [ ] First-order approximation stays as-is.
- [ ] Test confirms <1% breakdown voltage error vs iterated value.
- [ ] No convergence failures near breakdown.

---

## Part I — PHDL language features (generics, capabilities, bundles, enums, higher-order)

> This Part is the largest. It adds the spec's "few orthogonal concepts"
> (§1) that compose: generics, capabilities + operator sugar, bundles with
> methods, enum exhaustiveness, higher-order functions. Each is a
> distinct sub-part; they have dependencies noted.

### I.1 Expose `BundleDecl` in `ElabProgram`

**Spec:** §6.5 — bundles are a core concept.

**Status:** MISSING (data is lost). **High** (prerequisite for B.3, I.3,
I.6).

**Current state.** `Elaborator.bundles: HashMap<String, BundleDecl>`
(`lower.rs:20`) is used internally for port expansion but **not exposed in
`ElabProgram`** (`ir.rs:407-415`).

**Proposed solution.** Add `pub bundles: HashMap<String, BundleDecl>` to
`ElabProgram` and populate it in `elaborate` from the elaborator's table.

```rust
// elab/ir.rs
pub struct ElabProgram {
    pub modules: HashMap<String, ElabMod>,
    pub behaviors: Vec<ElabBehavior>,
    pub disciplines: HashMap<String, DisciplineDecl>,
    pub enums: HashMap<String, EnumDecl>,
    pub capabilities: HashMap<String, CapabilityDecl>,
    pub bundles: HashMap<String, BundleDecl>,   // <-- NEW
    pub functions: HashMap<String, ElabFn>,
    pub impls: Vec<ElabImpl>,
}
```

**Decision rationale.** Downstream (codegen, typecheck) needs bundle layout
for `BundleLit` construction, field-by-field connection, and method
dispatch. Today it's lost.

**Acceptance criteria.**
- [ ] `ElabProgram.bundles` added and populated.
- [ ] Existing tests green.
- [ ] `display.rs` can print bundle declarations.

---

### I.2 Capability dispatch: operators are sugar for capability methods

**Spec:** §6.6 — "Operators are sugar over standard capabilities — `a + b`
is `a.add(b)` — so implementing `Add` grants `+`."

**Status:** STUB. **High.**

**Current state.** Operators are kept as `Binary`/`Unary` exprs; no
desugaring to capability-method calls. The `impl Add for UInt[N]` bodies
are lowered but never dispatched.

**Proposed solution.** Add a *capability-resolution* pass in `piperine-lang`
(after elaboration, before codegen) that rewrites operator expressions into
method calls when the operand types implement the relevant capability.

1. Build a registry from `ElabProgram.impls`: `impl Add for UInt[N]` →
   `Add` is implemented by `UInt[N]` with method `add`.
2. Walk every expression in every behavior body and function body. For a
   `Binary(Add, a, b)` where `a`'s type implements `Add`, rewrite to
   `Call(Path("add"), [a, b])` (or a typed method-call form).
3. For primitive types (`Real`, `Integer`, etc.), the operators stay as
   `Binary` (they satisfy capabilities intrinsically — the spec says so).

**Decision rationale.** The spec's "operators are sugar" means the *type
checker* decides whether `+` is the primitive `+` or a method call. Doing
this at elaboration time (after types are known) is the cleanest place.
Primitive types keep the fast path.

**Why this depends on other Parts.** Needs type info (B.1) to know the
operand types. Needs `Self` handling (I.3) for the method bodies. Needs
`BundleLit` (I.6) for the `return UInt[N] { .bits = r }` in method bodies.

**Verification.** Test that `var x : UInt[8] = a + b;` where `a, b :
UInt[8]` rewrites to a call to `UInt::add`, and that the digital
interpreter executes it correctly.

**Acceptance criteria.**
- [ ] Capability registry built from `impls`.
- [ ] Operator-to-method rewrite for user types.
- [ ] Primitive types keep direct operators.
- [ ] `UInt[8] + UInt[8]` executes correctly in digital.
- [ ] Tests pass.

---

### I.3 `Self` substitution in `impl` method bodies

**Spec:** §6.5, §6.6 — `Self` is the implementing type.

**Status:** STUB. **High.**

**Current state.** `lower.rs:761`: `FnParam::SelfParam => None` — `Self`
is silently dropped from the elaborated param list. Methods lose their
receiver; no substitution of `Self` → implementing type.

**Proposed solution.**
1. In `elab_fn` (`lower.rs:753-791`), when lowering an `impl` method, keep
   `Self` as a typed param (the type is the implementing type, known from
   the `impl` header).
2. Substitute `Self` → implementing type name in the method body's type
   annotations and in the method's return type.
3. The method's `self` param becomes a by-reference (bundle) or by-value
   (primitive) first argument.

**Decision rationale.** `Self` is a type alias for the implementing type;
resolving it at elaboration is the standard approach.

**Verification.** Test that `impl Add for UInt[N] { fn add(self, o: Self)
-> Self { ... } }` elaborates with `self: UInt[N]`, `o: UInt[N]`, return
`UInt[N]`.

**Acceptance criteria.**
- [ ] `Self` kept as typed param in `impl` methods.
- [ ] `Self` substituted in body/return types.
- [ ] Test passes.

---

### I.4 Generic modules and bundles: `<T: Cap>` and `[N]`

**Spec:** §6.6 — "A `mod`, `bundle`, or `capability` is parameterized by
type in `<>` and by const in `[]`."

**Status:** STUB. **High** (blocks `Adder<T>`, `Pair<T>`, `UInt[N]` as
generic).

**Current state.**
- Const params `[N]` work (`lower.rs:826-878` monomorphizes on demand).
- Type params `<T: Cap>` are parsed (`ast.rs:71, 77-82`) but **discarded**
  at `lower.rs:441` (`type_args: _,`). `type_subst` is plumbed but always
  `&HashMap::new()`. Generic modules are **skipped** at `lower.rs:88`.
- `UInt[N]` works because `[N]` is a const param; `<T: Type>` would not.

**Proposed solution.** Implement type-parameter monomorphization:

1. **At instantiation** (`lower.rs:445-499`), when an instance supplies
   type arguments (e.g. `Adder <Logic>`), record them in a `type_subst`
   map.
2. **Monomorphize** the referenced module: produce a specialised copy
   with `T` replaced by `Logic` throughout (ports, vars, behavior bodies).
   Mangled name `Adder__Logic` (mirrors the const-param mangling
   `Foo__8` at `lower.rs:470-476`).
3. **Validate bounds**: for each `T: Cap` bound, check that the substituted
   type implements `Cap` (consult the impl registry from I.2).
4. **Cache** monomorphized modules in `mono_cache` (already exists for
   const mono).

**Implementation sketch:**

```rust
fn monomorphize_module(
    &mut self,
    decl: &ModDecl,
    type_args: &[(String, Type)],   // (param_name, substituted_type)
    const_args: &[(String, ConstVal)],
) -> Result<String, ElabError> {
    let mangled = mangle(&decl.name, type_args, const_args);
    if self.mono_cache.contains_key(&mangled) {
        return Ok(mangled);
    }
    let mut type_subst = HashMap::new();
    for (name, ty) in type_args {
        type_subst.insert(name.clone(), ty.name.clone());
    }
    // validate bounds
    for tp in &decl.type_params {
        let substituted = &type_subst[&tp.name];
        for bound in &tp.bounds {
            self.validate_bound(substituted, bound)?;
        }
    }
    // elaborate the module body with type_subst populated
    let elab_mod = self.elab_mod_inner(decl, &mut env, &type_subst)?;
    self.mono_cache.insert(mangled.clone(), elab_mod);
    Ok(mangled)
}
```

**Decision rationale.** Monomorphization is the spec's stated model
("compile-time by default" §1; "generic monomorphization" §1). It's the
simplest model and produces no runtime overhead. The const-param path
already does this; type-param is the same pattern with a `type_subst` map.

**Why this depends on other Parts.** Bound validation (I.5) needs the impl
registry. `Self` (I.3) needs to work first if generic bundles have methods.

**Verification.**
- Test `mod Adder <T: Add + Net> ( input a : T, input b : T, output y : T
  ); digital Adder { y <- a + b; }` instantiated as `Adder <Logic>` produces
  a specialised module `Adder__Logic` with `Logic` ports.
- Test bound violation: `Adder <Thermal>` (where `Thermal` does not impl
  `Add`) is rejected.

**Acceptance criteria.**
- [ ] `type_args` not discarded; `type_subst` populated.
- [ ] Generic modules monomorphized and cached.
- [ ] Bounds validated.
- [ ] `Adder<Logic>` test passes.
- [ ] Bound-violation test passes.

---

### I.5 Capability conformance check for `impl ... for`

**Spec:** §6.6 — "A type satisfies it through a separate `impl ... for`".

**Status:** MISSING. **Medium.**

**Current state.** `impl` blocks are lowered but no check that the impl
provides every signature the capability requires. Super-capabilities not
validated transitively.

**Proposed solution.** Add a `check_impl_conformance` pass:
1. For each `ElabImpl { type_name, capability_name, methods }`, look up
   the `CapabilityDecl`.
2. For each `CapItem::FnSig { name, sig }` in the capability, check that
   `methods` contains a method with the same name and compatible signature.
3. Recursively check super-capabilities: if the capability has supers
   (`Number : Add, Sub, Mul`), the type must also impl those (transitively).
4. Default bodies (`CapItem::FnDecl`) — if the capability provides a
   default, the impl may omit the method; the default is inherited.

**Decision rationale.** Conformance is the spec's "contract" promise.
Without it, an `impl Add for Foo` that omits `add` silently passes.

**Verification.** Test that an incomplete impl is rejected; a complete one
passes; a default-body method is inherited.

**Acceptance criteria.**
- [ ] `check_impl_conformance` pass exists.
- [ ] Missing-method error.
- [ ] Super-capability transitive check.
- [ ] Default-body inheritance.
- [ ] Tests pass.

---

### I.6 `BundleLit` construction and bundle value semantics

**Spec:** §6.5 — "A value is built with a literal `Name { .field = value }`,
and an omitted field takes its default."

**Status:** STUB. **High.**

**Current state.** `BundleLit` parsed (`parser.rs:891-928`) but not
const-evaluable (`const_eval.rs:99`), not constructed at codegen
(`codegen/expr.rs:61` unhandled), and `Self` is dropped (I.3). Bundle field
defaults (`FieldDecl.default`) captured but never applied.

**Proposed solution.**
1. In `const_eval`, evaluate a `BundleLit` to a `ConstVal::Bundle(HashMap<
   String, ConstVal>)` by evaluating each field expr and applying defaults
   for omitted fields (from `BundleDecl.fields[i].default`).
2. In the codegen, a `BundleLit` in a digital context becomes a field map
   the interpreter can read/write. In an analog context (rare — bundles are
   usually digital), lower each field separately.
3. Apply bundle defaults in port expansion (`lower.rs:301-310`) and in
   `BundleLit` construction.

**Decision rationale.** Bundle literals are the spec's constructor. Defaults
are the spec's "omitted field takes its default". Both are needed for
`UInt[N] { .bits = r }` in `impl` method bodies.

**Verification.** Test `var c : Complex = Complex { .re = 1.0, .im = 0.0 };`
elaborates with `c.re = 1.0`, `c.im = 0.0` (default). Test
`UInt[8] { .bits = r }` constructs.

**Acceptance criteria.**
- [ ] `BundleLit` const-evaluable.
- [ ] Bundle defaults applied.
- [ ] Codegen handles `BundleLit`.
- [ ] Tests pass.

---

### I.7 Enum discriminant evaluation and `match` exhaustiveness

**Spec:** §6.4, §8.3 — enums with optional `: Repr`, sequential/explicit
values; `match` checked for exhaustiveness.

**Status:** PARTIAL. **Medium.**

**Current state.** `EnumVariant.value: Option<Expr>` captured but never
const-evaluated; no auto-increment; no exhaustiveness check. `Pattern::
Wildcard` parsed but unused.

**Proposed solution.**
1. In `elaborate`, evaluate enum variant discriminants: sequential from 0
   if no explicit value; explicit values from the `Expr`; validate the
   repr type is a digital net type (`Bit[ceil(log2(count))]` default).
2. Add `check_match_exhaustiveness` to the typecheck pass: for each `match
   over EnumType`, collect the variant set; check that the arms cover all
   variants (or have a wildcard).

**Decision rationale.** Exhaustiveness is the spec's "checked for
exhaustiveness" promise (§8.3). Discriminant evaluation is needed for
`match` to actually compare values.

**Verification.** Test a non-exhaustive `match` is rejected; an exhaustive
one passes; explicit-value enum compares correctly.

**Acceptance criteria.**
- [ ] Enum discriminants const-evaluated.
- [ ] Repr type validated.
- [ ] `match` exhaustiveness checked.
- [ ] Tests pass.

---

### I.8 Higher-order functions: lambdas, `map`/`reduce`, bounded recursion

**Spec:** §7.1 — "A function is a value ... a lambda `|a, b| a + b` is an
anonymous one ... `reduce(parts, |a, b| a + b)` emits a balanced adder
tree ... recursion is resolved entirely at elaboration and must terminate".

**Status:** STUB/MISSING. **Medium** (advanced feature).

**Current state.** `Lambda` parsed (`parser.rs:815-830`) but not reduced at
elab; rejected at codegen (`ir_emit.rs:473`). `map`/`reduce` in prelude but
with generic-stubbed bodies. No recursion in `const_eval` (`Expr::Call` arm
absent). No depth limit.

**Proposed solution.**
1. **Lambda capture:** add a capture-analysis pass — a lambda may capture
   only elaboration constants (the spec's rule). Enforce it.
2. **`map`/`reduce` execution:** at elaboration, when `map`/`reduce` is
   called with a lambda and a const-sized array, *evaluate* it: apply the
   lambda to each element (the lambda body is an `IrExpr` with captured
   consts substituted), producing a new const array. This is the spec's
   "generation by evaluation, with nothing expanded" — the elaboration
   *runs* the function.
3. **Bounded recursion:** add an `Expr::Call` arm to `const_eval` that
   evaluates user `fn` calls recursively, with a hard depth counter (e.g.
   256) — the spec's "hard depth limit as a backstop". Each recursive call
   must reduce a const parameter (the spec's termination rule); detect
   non-reduction and error early.

**Implementation sketch for `const_eval`:**

```rust
// const_eval.rs — add Call arm:
Expr::Call(func, args) => {
    let func_name = match func.as_ref() {
        Expr::Ident(n) => n.clone(),
        Expr::Path(segs) if segs.len() == 2 => format!("{}::{}", segs[0], segs[1]),
        _ => return Err(ConstError::NotConst("call target must be a name".into())),
    };
    // look up the fn in the elaborator's function table
    let fn_decl = self.functions.get(&func_name)
        .ok_or_else(|| ConstError::NotConst(format!("unknown function `{func_name}`")))?;
    // evaluate args
    let arg_vals: Vec<ConstVal> = args.iter()
        .map(|a| self.eval(a)).collect::<Result<_,_>>()?;
    // bind params
    let mut inner_env = self.clone();
    for (p, v) in fn_decl.params.iter().zip(arg_vals.iter()) {
        inner_env.bind(p.clone(), v.clone());
    }
    inner_env.depth += 1;
    if inner_env.depth > 256 {
        return Err(ConstError::NotConst("recursion depth exceeded".into()));
    }
    // evaluate the body, returning the Return expr
    inner_env.eval_fn_body(&fn_decl.body)
}
```

**Decision rationale.** The spec's "generation by evaluation" means the
elaboration phase is a total evaluator. Lambdas are pure; captured consts
keep it total. The depth cap is the backstop the spec explicitly requires.

**Why this depends on other Parts.** Needs D.5 (function inlining
machinery) for the codegen side; the elaboration-side evaluation is
independent but shares the `fn` table.

**Verification.**
- Test `map([1, 2, 3], |x| x * 2)` elaborates to `[2, 4, 6]`.
- Test `reduce` builds a balanced tree (inspect the IR).
- Test a recursive `factorial(N)` with `N=5` evaluates to 120.
- Test `factorial(N)` with `N` non-const is rejected (recursion must be
  resolved at elaboration).
- Test recursion past the depth cap errors clearly.

**Acceptance criteria.**
- [ ] Lambda capture analysis enforces const-only.
- [ ] `map`/`reduce` evaluated at elaboration.
- [ ] Bounded recursion with depth cap.
- [ ] Non-const recursion rejected.
- [ ] Tests pass.

---

### I.9 `var` in `mod` body is not silently dropped

**Spec:** §5.2 — `var` is a storage class in a `mod` body (for state held
across the module's behavior).

**Status:** STUB. **Medium.**

**Current state.** `lower.rs:399-401`:

```rust
// Vars in mod body appear in behavior; skip at structural level.
```

`var` decls in a `mod` body are silently dropped at structural elaboration.
They survive only inside `analog`/`digital` blocks.

**Proposed solution.** Carry mod-body `var` decls into the module's
`ElabMod` as module-level state, visible to both `analog` and `digital`
blocks. The spec's `SarAdc` (B.1) declares `var state : SarState = Idle;`
at mod-body level and uses it in the `digital` block.

**Decision rationale.** Mod-body `var` is module state, shared across
behaviors. Dropping it forces the user to re-declare inside each block,
which is not the spec's syntax.

**Verification.** Test that `mod M { var s : Bit = 0; digital M { ... s
... } }` elaborates with `s` visible in the digital body.

**Acceptance criteria.**
- [ ] Mod-body `var` decls carried into `ElabMod`.
- [ ] Visible to analog/digital blocks.
- [ ] Test passes.

---

### I.10 `pub` visibility enforced

**Spec:** §4 — "An item is private unless marked `pub`, and `use pkg::item`
imports a public item."

**Status:** STUB. **Low.**

**Current state.** `is_pub` flag captured on every declaration (`ast.rs:
38-49`) but never checked. Private items are freely `use`able.

**Proposed solution.** In the resolver (`resolve/mod.rs`), when resolving
`use foo::bar`, check that `bar` is `pub` in `foo`. A non-pub item used
cross-package is `Err("item `{bar}` is private in `foo`")`.

**Decision rationale.** The spec's visibility rule. Without it, the
`pub` keyword is decorative.

**Verification.** Test that `use foo::private_item` errors; `use
foo::pub_item` succeeds.

**Acceptance criteria.**
- [ ] `pub` enforced on `use` resolution.
- [ ] Tests pass.

---

### I.11 `&&` / `||` not in PHDL grammar; NGSPICE models use `ng_and`/`ng_or` helpers

**Spec:** Implied by §6.1 ("Boolean: Two-state logic value (0, 1)") — the
spec needs logical connectives to write guards like
`if (model.ikf > 0 && cd > 1e-18)`.

**Status:** STUB. **Low** (workaround exists; affects ergonomics and
faithfulness of the NGSPICE headers).

**Cross-reference:** `docs/NGSPICE_GAPS.md` P.1; the faithful
`crates/piperine-lang/headers/ngspice/ngspice_constants.phdl` defines
`ng_and`/`ng_or` pure fns because `&&`/`||` don't parse.

**Current state.** The lexer recognizes `&&`/`||` as `Tok::And`/`Tok::Or`
(`lexer.rs:101-104, 239-242`); the comment says *"lexed for error clarity,
not in PHDL grammar."* The expression table at `expr.rs:45-55` lists
operators at precedences 1 (BitOr) through 7 (Mul/Div/Rem) but **omits**
`And` and `Or`.

A model like `if (model.ikf > 0.0 && cd > 1.0e-18) { ... }` produces the
parse error `"Expected RParen, found Some(And)"` — the `&&` is lexed but
the expression parser has no rule for it.

**Why it matters.** Every NGSPICE faithful model (res/dio/bjt/jfet/mos1)
uses compound conditions. The `ng_and(a, b)` / `ng_or(a, b)` helpers
work but:
- Look unnatural next to the Verilog-A / AMS original.
- Require call overhead (if not inlined).
- Don't short-circuit (`(a > 0)` is always evaluated even when `a == 0`),
  which the spec's `&&` would do.

**Proposed solution.** Two options.

**Option A (operator form):** add `And` and `Or` to the binary-op
precedence table at level 1 (just below `BitOr`) and 2 (just below
`BitAnd`). Extend `validate_ir_contrib` to accept them in digital/analog
contributions.

```rust
// expr.rs:45-55
Some(Tok::And) => Some((BinaryOp::And, 0)),   // lowest precedence
Some(Tok::Or)  => Some((BinaryOp::Or, 0)),
```

`BinaryOp::And`/`Or` already exist in the IR (`ir.rs`). Add symbolic
derivatives in `ir_emit.rs:diff_call` (they short-circuit on constants
but here the operands are always real-valued `Boolean` so the derivative
simplifies).

**Option B (keyword form):** accept `and`/`or` as aliases. Same as A but
without changing operator precedence. Matches Mathematica/Rust bool
syntax.

**Decision rationale.** Prefer **Option A** — `&&` and `||` are the
textbook forms users expect. The precedence slot (level 1, lowest)
mirrors C/Python/Java semantics.

**Verification.**
- `if (a > 0 && b > 0) { ... }` parses and works.
- `if (a > 0 || b > 0) { ... }` parses and works.
- NGSPICE faithful `diode.phdl` removes the `ng_and`/`ng_or` wrappers
  and uses `&&`/`||` directly.
- Short-circuit test: `if (false && true_branch())` — `true_branch()` is
  not called. (Optional; the JIT may not short-circuit at runtime; if so,
  document.)

**Acceptance criteria.**
- [ ] `And` and `Or` accepted in the expression table.
- [ ] Symbolic derivatives registered.
- [ ] NGSPICE diode model E2E with `&&`/`||` (no helper fns) passes.
- [ ] `cargo test -p piperine-codegen` green.

---

### I.12 `else if` not supported in if-expressions (only in statements)

**Spec:** §8 — if-expressions are part of the analog/digital surface.

**Status:** STUB (works in statements, fails in expressions). **Low.**

**Cross-reference:** `docs/NGSPICE_GAPS.md` P.2.

**Current state.** Two independent parsers handle `if`:
- **If-expression** (`expr.rs:94-102`): after `else`, calls
  `parse_block()` which expects `{`. So `else if (B) { Y }` errors with
  `"Expected LBrace, found Some(Ident(\"if\"))"`.
- **If-statement** (`stmt.rs:211-235`): explicitly handles `else if` (line
  222, `if self.eat_ident("if")`).

The asymmetry surfaces in NGSPICE faithful `passives.phdl` and `mos.phdl`,
which declare `var r_nom : Real = if (r_given) { r } else if (geometry) {
… } else { … };` in expression context. The headers were converted to the
nested `else { if (B) { … } }` form by hand (with explicit `}` per chain
level) — this is noisy and error-prone.

**Why it matters.** Spec ergonomics. Every chained if-expression in a
`var` initializer needs manual restructuring. A two-branch chain is
tolerable; a four-branch chain (`if A elif B elif C elif D else E`) turns
into four nesting levels of `{ }`.

**Proposed solution.** In `expr.rs:100-101`, after consuming `else`, check
if the next token is `if` and parse a nested if-expression in-place:

```rust
self.expect_ident_str("else")?;
let else_body = if self.peek_ident("if") {
    // parse inner `if (cond) { ... } else { ... }` as an expression
    self.pos += 1;  // skip the `if`
    self.expect(&Tok::LParen)?;
    let inner_cond = self.parse_expr()?;
    self.expect(&Tok::RParen)?;
    let inner_then = self.parse_block()?;
    self.expect_ident_str("else")?;
    let inner_else = self.parse_block()?;
    Expr::If { cond: inner_cond, then_body: inner_then, else_body: inner_else }
} else {
    self.parse_block()?
};
```

**Decision rationale.** Single-line change in `parse_primary`. The
else-if chain becomes a nested `Expr::If` tree, which the codegen and
elaborator already handle (they recurse on `if` expressions). No new IR
variant.

**Verification.**
- `var x : Real = if (A) { X } else if (B) { Y } else { Z };` parses.
- The chain produces the same IR/behavior as the equivalent
  `if (A) { X } else { if (B) { Y } else { Z } }`.
- NGSPICE faithful `passives.phdl` removes the explicit `}` per chain
  level.

**Acceptance criteria.**
- [ ] `else if` accepted in if-expression context.
- [ ] Backward compatible: `if/else` in statement context still works
      (separate parser at `stmt.rs:211`).
- [ ] NGSPICE `passives.phdl` `var r_nom` uses `else if` without manual
      nesting.
- [ ] `cargo test -p piperine-codegen` green.

---

### I.13 `sinh` / `cosh` / `tanh` not built-in (spec §8.1 promises `tanh`)

**Spec:** §8.1 — "Built-ins: ... the math functions (`exp`, `ln`, `sqrt`,
`pow`, `tanh`, …)." The spec promises `tanh` and lists the rest.

**Status:** MISSING from builtins. **Low** (workaround exists).

**Cross-reference:** `docs/NGSPICE_GAPS.md` P.3; `ngspice_constants.phdl`
defines them as `fn` because the codegen lacks them.

**Current state.** The builtin math table
(`codegen/cranelift_helpers.rs:22-51`, `codegen/analog.rs:33-73`)
registers 18 functions: `exp`, `ln`/`log`, `log10`, `sqrt`, `abs`,
`sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `pow`, `min`,
`max`, `floor`, `ceil`, `limexp`. Spec promises (`sinh`, `cosh`,
`tanh`) are missing.

The BJT base-resistance IRB formula in NGSPICE uses `tan()` (which IS
present), but `tanh` would be needed for accurate high-current β
roll-off. Several compact models use `tanh` for soft saturation.

**Why it matters.** Faithful BJT IRB fidelity, future compact-model work,
spec compliance. User workarounds (`(exp(2x)-1)/(exp(2x)+1)` for `tanh`)
defeat the spec's "linearise / differentiate for Jacobian automatically"
promise — the compiler can compute `∂tanh/∂x = sech²(x)` symbolically but
cannot for the hand-rolled expression.

**Proposed solution.** Add three entries to the registry:

```rust
// codegen/cranelift_helpers.rs:22-51
"sinh"  => emit_call_sinh,    // (exp(x) - exp(-x)) / 2
"cosh"  => emit_call_cosh,    // (exp(x) + exp(-x)) / 2
"tanh"  => emit_call_tanh,    // library call (libm tanhf on f32; f64 via tanh)
```

Symbolic derivatives in `ir_emit.rs:346-378`:
- `∂sinh/∂x = cosh(x)`
- `∂cosh/∂x = sinh(x)`
- `∂tanh/∂x = 1 − tanh²(x)` (compute as `(1 − f²)` from the cached `tanh` result)

**Decision rationale.** libm wrappers are cheap, derivatives are pure
2-liner additions.

**Verification.**
- `sinh(1.0)` ≈ 1.17520.
- `cosh(1.0)` ≈ 1.54308.
- `tanh(1.0)` ≈ 0.76159.
- Derivatives: `d(2*x)/d(x) = 2`; check via finite-difference at `x=1.0`:
  `tanh(1.001) − tanh(0.999) ≈ 0.001 × (1 − tanh²(1)) ≈ 0.001 × 0.42`.

**Acceptance criteria.**
- [ ] `sinh`, `cosh`, `tanh` as builtins.
- [ ] Derivatives registered.
- [ ] `ngspice/headers/ngspice_constants.phdl` removes its local `fn`s
      (they become duplicates).
- [ ] `cargo test -p piperine-codegen` green.

---

### I.14 Bundle-typed `param`: elaboration lowers individual slots, not the bundle

**Spec:** §6.5 — "A bundle is net-capable when every field is a net type...
otherwise a `param`/`var`." The `.MODEL` / instance parameter separation is
the central reason for PHDL's bundle-param syntax.

**Status:** STUB (parse-only, no tested elaboration). **Medium.**

**Cross-reference:** `docs/NGSPICE_GAPS.md` P.4; every NGSPICE model
declares `param model : XxxModel = XxxModel {}` — this gap is the blocker
for the NGSPICE faithful headers to compile through elaboration.

**Current state.** The parser accepts `param model : DioModel =
DioModel {}` (verified by `crates/piperine-lang/tests/ngspice_parse_tests.rs`).
The elaboration in `elab/lower.rs:825-878` lowers bundle fields but the
*instance-to-bundle binding* path has no tested path: when a parent
writes `R ( a, b ) { .model = ResModel { .rsh = 50.0 } }`, the
elaborator has to:
1. Construct a `ResModel` value from the literal.
2. Merge it with the instance's default (each field individually).
3. Expose each field as a separate readable name (`model.rsh`) in
   analog/digital bodies.

Steps 1–3 are partially implemented but unexercised.

**Why it matters.** The PHDL spec's `.MODEL` semantics are the cleanest
encoding of the SPICE model/instance split. Without full bundle-param
support, PHDL users either repeat all model params per instance (verbose,
error-prone) or use sentinel defaults (fragile — see A.15).

For the NGSPICE faithful headers in particular, every device needs:
```phdl
mod dio (...) {
    param model : DioModel = DioModel {};
    analog dio { ... model.is * area_eff ... }
}
```

`model.is` is a bundle field access inside the analog body. Without I.14,
this fails at elaboration; the NGSPICE headers are parse-only.

**Proposed solution.** Two changes in `elab/lower.rs`:

1. **At parse/elab of `param` with bundle type:**
   - Look up the bundle declaration.
   - For each field `f` with default `d_f`, create an internal slot
     `model_f` initialised to `d_f`.
   - The analog body sees `model.rsh` → resolves to slot `model_rsh`.

2. **At instance binding** (the parent's `.model = ResModel {…}` literal):
   - Evaluate the literal into a `HashMap<String, ConstVal>`.
   - For each field in the literal, update the corresponding slot.
   - Fields omitted from the literal keep their default.
   - If the literal has fields not declared in the bundle → `ElabError`.

```rust
// elab/lower.rs — sketch:
fn lower_bundle_param(
    env: &mut LowerEnv,
    pname: &str,
    bundle_name: &str,
    fields: &HashMap<String, ConstVal>,  // instance override; None for default
) -> Result<(), ElabError> {
    let bundle = env.bundles.get(bundle_name)
        .ok_or_else(|| ElabError::UnknownBundle(bundle_name.into()))?;
    for (fname, fdef) in &bundle.fields {
        let value = match fields.get(fname) {
            Some(v) => v.clone(),
            None    => env.eval_const(&fdef.default)?,
        };
        env.declare(pname, fname, value);  // creates `model.fname` binding
    }
    Ok(())
}
```

**Decision rationale.** Bundle literals are the spec's constructor (§6.5).
Defaults are the spec's "omitted field takes its default". Both are needed
for `UInt[N] { .bits = r }` in method bodies (I.6) AND for
`DioModel { .is = 1e-12 }` in NGSPICE instance overrides. Same machinery
serves both; do it once.

**Why this depends on other Parts.** I.6 (`BundleLit` const-eval) and the
existing I.1 (`ElabProgram.bundles`) are prerequisite — you need the bundle
layout to apply defaults.

**Verification.**
- `mod dio ( ...) { param model : DioModel = DioModel {};
  analog dio { var x : Real = model.is; } }` elaborates with
  `x = 1e-14` (default).
- Override: parent writes `d1 : dio ( p, n ) { .model = DioModel { .is =
  1e-12 } }`; analog body sees `model.is = 1e-12`.
- Negative: override field not in bundle → elaboration error.
- NGSPICE faithful passives.phdl test: instance with no overrides uses
  `model.rsh` (geometric computation); override `.rsh = 50.0` uses 50.

**Acceptance criteria.**
- [ ] `param` of bundle type elaborates; each field becomes a named slot
      with the default value.
- [ ] Override literal merges field-by-field.
- [ ] Field access in analog body resolves to slot.
- [ ] NGSPICE faithful `diode.phdl` elaborates end-to-end (with I.14 +
      A.14 + A.15 + D.8 + D.10 unblocking codegen too).
- [ ] `cargo test -p piperine-codegen -p piperine-solver` green.

---

### I.15 Module-level `var` is persistent analog state (switch hysteresis, etc.)

**Spec:** §5.2 — "`var`: A mutable binding. In a `digital` block it is
combinational unless it must hold a value, when it infers memory." The spec
doesn't explicitly address `var` in module body (top-level scope), but
behavioral `var` is explicitly stateful, and the spec's Appendix B.1
(`SarAdc`) and the NGSPICE faithful headers depend on this.

**Status:** STUB (mod-body `var` is silently dropped per `lower.rs:399-401`).
**Medium.**

**Cross-reference:** `docs/NGSPICE_GAPS.md` P.5; `ngspice/switches.phdl`
declares `var sw_state : Real = 0.0` in the module body.

**Current state.** The elaborator at `lower.rs:399-401`:

```rust
// Vars in mod body appear in behavior; skip at structural level.
```

silently drops module-level `var` declarations. Inside `analog`/`digital`
blocks, `var` is local (recomputed each iteration). There is no
mechanism for a `var` that survives across iterations *or* across
accepted time steps.

The NGSPICE faithful `sw` and `csw` models declare their state at module
level:

```phdl
mod sw (inout p, n, cp, cn) {
    param model : SwModel = SwModel {};
    param off : Boolean = 1;
    var sw_state : Real = 0.0;   // <-- silently dropped by current elaborator
}
analog sw {
    var vc : Real = V(cp, cn);
    @ above(vc - (vt + vh)) { sw_state = 1.0; }   // <-- writes to dropped slot
    ...
}
```

The spec's `SarAdc` (Appendix B.1) does the same with `var state :
SarState = Idle;` — state shared across analog and digital blocks.

**Why it matters.** Without persistent `var`:
- Hysteresis machines cannot remember their previous state.
- State shared between analog and digital blocks (the most common pattern
  in mixed-signal) is inexpressible.
- The PHDL/Ngspice-test headers for `sw`/`csw` cannot be elaborated; the
  analog event handlers have no stable target to write to.

**Proposed solution.** Two parts:

1. **Module-body `var` is device state.** In `lower.rs:399`, instead of
   skipping, hoist `var` decls into `ElabMod.state: Vec<IrVarDecl>` (a new
   field). Each occupies one slot in the device's runtime state vector
   (allocated alongside `ddt`/`idt` StateRefs).

2. **Event-handler writes are persistent.** In the codegen, when a
   `@ above(...) { sw_state = 1.0; }` handler runs, the write updates
   the device state slot. The analog body reads the *current* (post-event)
   value. A run-time check: if an event handler updates a `var`, does the
   analog body read it after the event? Yes (this is the spec's intent).

The implementation reuses the `StateRef` allocation machinery used for
`ddt`/`idt` (`ir_emit.rs:88-125`). A `var` becomes a new
`IrStateKind::Var { name, initial }`.

3. **The analog body reads it normally:**
   ```phdl
   var g : Real = if (sw_state > 0.5) { gon } else { goff };
   ```
   On the first iteration, `sw_state = 0.0` (initial). After an `@ above`
   fires, `sw_state = 1.0` for subsequent iterations until the next event
   resets it.

**Decision rationale.** Mod-body `var` is device state — the same
abstraction as `ddt`'s integrator value or `Idt`'s accumulator. Using the
same StateRef mechanism keeps the storage uniform. The alternative
(global variables) couples modules in ways the spec forbids.

**Why this depends on other Parts.** E.4/E.5 (analog events): the event
handler must actually fire for `sw_state` to be written. Until E.4 lands,
`sw_state` stays at its initial value forever and the switch never
transitions — useful as a partial test (the parse/elaborate step) but
not for full E2E.

**Verification.**
- Module with `var x : Real = 0; analog M { ... x ... }` elaborates
  without dropping `x`.
- The `sw` model: state is at initial value (0) before any event;
  update via `sw_state = 1.0;` in an event handler makes subsequent
  analog iterations read 1.0.
- NGSPICE faithful `switches.phdl` elaborates end-to-end (modulo E.4
  for event firing).

**Acceptance criteria.**
- [ ] Mod-body `var` hoisted into `ElabMod.state`.
- [ ] `ElabProgram` exposes state vector.
- [ ] Codegen reads/writes the state slot; survives across Newton
      iterations and across accepted time steps.
- [ ] NGSPICE `switches.phdl` static semantics pass (state exists even
      if events don't fire yet).
- [ ] `cargo test -p piperine-codegen` green.

---

## Part J — Diagnostics, events, `$assert`

### J.1 `$bound_step` / `$analysis` are not `Diagnostic`s

**Spec:** §8.2 — `$bound_step(dt)`; §8 — `$analysis` returns an `Analysis`
enum.

**Status:** WRONG CATEGORY. **Medium.**

**Current state.** `piperine-lang/src/parse/parser.rs:526-543` lumps every
`$ident(...)` into `BehaviorStmt::Diagnostic { sys, args }`. `$bound_step`
is a system task that caps the next solver step; `$analysis` returns an
enum value. They are not diagnostics.

**Proposed solution.** Add distinct AST variants:
- `BehaviorStmt::BoundStep(IrExpr)` — for `$bound_step(dt)`.
- `BehaviorStmt::AnalysisQuery` — for `$analysis` (or handle it as a
  `SimQuery` variant in the IR, see below).

At IR lowering, `$bound_step(dt)` → `IrStmt::BoundStep(IrExpr)` (the IR
already has this variant, `ir.rs:386`). `$analysis("tran")` →
`IrExpr::Sim(SimQuery::Analysis("tran"))` (the IR already has this, `ir.rs:
309`). The `Diagnostic` variant is reserved for `$display`/`$warning`/
`$error`/`$fatal`/`$info`.

**Decision rationale.** Mis-categorising system tasks as diagnostics loses
semantics. The IR already has the right variants; the parser just needs to
dispatch correctly.

**Verification.** Test that `$bound_step(1e-9)` in an analog block lowers to
`IrStmt::BoundStep`, not `Diagnostic`. Test that `$analysis("tran")` lowers
to `SimQuery::Analysis`.

**Acceptance criteria.**
- [ ] `$bound_step` and `$analysis` have distinct AST/IR paths.
- [ ] `Diagnostic` reserved for actual diagnostics.
- [ ] Tests pass.

---

### J.2 `$assert(cond, msg)` is a real assertion, not a `Diagnostic`

**Spec:** §8.5 — "`$assert(cond, msg)` reports when `cond` is false. In `@
initial` it validates setup: `@ initial { $assert(n > 0, "n>0"); }`."

**Status:** PARTIAL. **Medium.**

**Current state.** `$assert` is just another `BehaviorStmt::Diagnostic` with
`sys = "assert"`. No special cond/msg treatment; no `@ initial` validation.

**Proposed solution.**
1. Add a distinct AST variant `BehaviorStmt::Assert { cond, msg }`.
2. At IR lowering, `Assert { cond, msg }` → `IrStmt::Diagnostic { severity:
   Severity::Error, format: "assertion failed: {msg}: cond={cond}", args:
   [cond] }` for the general case, BUT in an `@ initial` context, evaluate
   `cond` at elaboration (if it's a const expr) and fail elaboration if
   false — the spec's "validates setup" semantics.

**Decision rationale.** `$assert` in `@ initial` is the spec's
static-validation mechanism. Making it an elaboration-time check (when the
cond is const) catches setup errors before simulation.

**Verification.** Test `@ initial { $assert(n > 0, "n>0"); }` with `n = 0`
fails elaboration; with `n = 5` passes. Test runtime `$assert` (non-const
cond) reports at simulation time.

**Acceptance criteria.**
- [ ] `Assert` distinct AST variant.
- [ ] `@ initial` const-cond assertions evaluated at elaboration.
- [ ] Runtime assertions report at sim time.
- [ ] Tests pass.

---

### J.3 `$error`/`$warn`/`$info` validated and tested

**Spec:** §8.5.

**Status:** PARTIAL. **Low.**

**Current state.** All `$ident(...)` collapse into `Diagnostic` without
validating the name. No test exercises any diagnostic.

**Proposed solution.** In the parser, restrict the recognized diagnostic
names to `$display`/`$write`/`$strobe`/`$monitor`/`$warning`/`$error`/
`$fatal`/`$info`. An unknown `$foo(...)` is an error
`"unknown system task `$foo`"`.

**Verification.** Test that each recognized name lowers to the right
`Severity` (`$error`/`$fatal` → Error/Fatal, `$warning` → Warning, others
→ Info). Test unknown names error.

**Acceptance criteria.**
- [ ] Diagnostic name validated.
- [ ] Severity mapping correct.
- [ ] Tests pass.

---

### J.4 Casts `real(x)`/`int(x)`/`bit(x)` recognized as casts

**Spec:** §6.1 — "casts are otherwise explicit (`real(x)`, `int(x)`,
`bit(x)`)."

**Status:** STUB. **Medium.**

**Current state.** Casts parse as ordinary `Expr::Call(Expr::Ident("real"),
[x])`. No cast semantics, no validation that the target is a valid cast.

**Proposed solution.**
1. In the parser, recognise `real(x)`/`int(x)`/`bit(x)`/`quad(x)` as a
   `Expr::Cast(target_ty, x)` (new variant) rather than a generic call.
2. At typecheck (B.5), validate that the cast is one of the allowed
   coercions (`real(x)` from `Integer`/`Boolean`/`Quad`, `int(x)` from
   `Real`/`Boolean`, `bit(x)` from `Integer`/`Quad`, etc.).
3. At codegen, lower each cast to the appropriate conversion (`as f64`,
   `as i64`, bit extraction).

**Decision rationale.** Casts are a distinct semantic operation, not a
function call. Recognising them in the parser lets the typechecker enforce
the allowed coercions (the spec's "casts are otherwise explicit").

**Verification.** Test `real(5)` → `5.0`; `int(3.7)` → `3` (or `4` per
rounding rule — define it); `bit(1)` → `1`. Test an illegal cast
(`real("hello")`) is rejected.

**Acceptance criteria.**
- [ ] `Expr::Cast` variant added.
- [ ] Allowed coercions enforced.
- [ ] Codegen lowers casts correctly.
- [ ] Tests pass.

---

## Part K — Architecture cleanup

> These are not spec gaps but code-health issues that affect correctness
> and maintainability. Do them after the silent bugs (Part A) but they can
> run in parallel with Parts B–J.

### K.1 Deprecate the `from_elab` analog path; route everything through IR

**Spec:** N/A (internal).

**Status:** LIABILITY. **High.**

**Current state.** Two parallel analog paths in `piperine-codegen`:
- `from_elab` (`codegen/analog.rs:147-193`, `autodiff.rs`) — PHDL `Expr` →
  JIT. Does NOT support `ddt` (react_contributions empty, A.7). Has silent
  fallbacks in `expr.rs:60-63, 100-102, 124-131` with no `validate_*` guard.
- `compile_analog_module_ir` (`codegen/analog.rs:326-341`, `ir_emit.rs`) —
  `IrExpr` → JIT. Has `validate_ir_contrib` fail-loud. Supports `ddt`.

**Why it matters.** The `from_elab` path is a silent-wrong-code surface
(see A.7). Two paths is a maintenance liability and a divergence risk.

**Proposed solution.**
1. Audit callers of `compile_analog_module` (the `from_elab` entry). Per
   the explore report: only `from_elab.rs` and tests.
2. Migrate callers to `compile_analog_module_ir` via `ppr_to_ir` +
   `ir_analog_to_device`.
3. Once no caller uses `from_elab`, mark `compile_analog_module` and
   `autodiff.rs` as `#[deprecated]` for one release, then remove.
4. Remove the silent fallbacks in `expr.rs` (they become unreachable once
   `from_elab` is gone).

**Decision rationale.** The IR path is the strategic future (it's the
contract with both frontends, has validation, supports reactive). Keeping
a second path that silently drops `ddt` and has no validation is a
liability. The migration is mechanical (route through `ppr_to_ir`).

**Verification.** All existing analog tests pass through the IR path after
migration. `cargo test -p piperine-codegen` green.

**Acceptance criteria.**
- [ ] No caller of `compile_analog_module` outside tests.
- [ ] `compile_analog_module` / `autodiff.rs` deprecated.
- [ ] Silent fallbacks in `expr.rs` removed.
- [ ] `cargo test -p piperine-codegen` green.

---

### K.2 `IrFunction` table is read by codegen or removed

**Spec:** §7 — `fn` inlines at the call site.

**Status:** DEAD DATA. **High.**

**Current state.** `IrProgram.functions` and `IrModule.functions` populated
by both frontends but read by no codegen file (only `display.rs`).
`IR-SYSTEM.md:22` falsely claims the codegen resolves user functions.

**Proposed solution.** This is resolved by D.5 (inlining). If D.5 is not
done, the alternative is to **remove the `IrFunction` fields** and fail at
IR lowering with `"user functions not yet supported"` — a cleaner
fail-loud than dead data. Prefer D.5.

**Decision rationale.** Dead data in the IR is a contract violation
(IR-SYSTEM §1.4 says the codegen resolves). Either fulfill the contract
(D.5) or remove the field and update the doc.

**Acceptance criteria.**
- [ ] Either D.5 done (inlining) or `IrFunction` removed.
- [ ] `IR-SYSTEM.md:22` updated to reflect reality.
- [ ] No dead IR fields.

---

### K.3 `BundleDecl` exposed in `ElabProgram` (prerequisite for B.3, I.3, I.6)

**Spec:** §6.5.

**Status:** MISSING. **High.**

(See I.1 for full detail. Listed here as an architecture item because it's
a data-loss bug in the elaborator, not a spec feature.)

---

### K.4 IR carries structured net refs instead of flat strings

**Spec:** N/A (internal).

**Status:** LIABILITY. **Medium.**

**Current state.** `IrConnection { port: Option<String>, net: String }`,
`IrConnectionDecl { lhs: String, rhs: String }`, `BranchAccess { plus:
String, minus: String }` — all flat strings. Hierarchical refs (`name.port`,
`name[i].port`) are parsed as strings and re-parsed downstream.

**Proposed solution.** Introduce a structured `IrNetRef` type:

```rust
pub enum IrNetRef {
    Simple(String),                  // "p"
    Indexed { base: String, idx: u32 },  // "p[3]"
    Field { base: String, field: String }, // "load.p"
    IndexedField { base: String, idx: u32, field: String }, // "rseg[0].n"
}
```

Replace the flat `String` fields in `IrConnection`, `IrConnectionDecl`,
`BranchAccess`, `Contrib`, `Force` with `IrNetRef`. The `from_ir` resolver
(F.3) handles them structurally.

**Decision rationale.** Flat strings force every consumer to re-parse. A
structured type makes hierarchical refs first-class and eliminates the
parsing ambiguity. This is a prerequisite for F.3 (hierarchical refs).

**Verification.** Existing IR tests updated to expect `IrNetRef` values.
`cargo test -p piperine-codegen` green.

**Acceptance criteria.**
- [ ] `IrNetRef` enum added.
- [ ] IR connection/branch fields use `IrNetRef`.
- [ ] `from_ir` resolves `IrNetRef` structurally.
- [ ] `display.rs` prints `IrNetRef` correctly.
- [ ] Tests pass.

---

### K.5 `Port` enum: fix the dangling doc reference or consume it

**Spec:** N/A.

**Status:** DOC LIES. **Low.**

**Current state.** `crates/piperine-solver/src/port.rs:1` cites "Section 3
of SOLVER_COSIMULATION.md"; no such file exists. The `Port` enum is
described as "single type used across compiler, elaborator, and solver" but
the solver never consumes it (works in `AnalogReference`/`DigitalNet`).

**Proposed solution.** Two options:
1. **Make the doc true:** actually use `Port` in the solver's public API
   (e.g. `CircuitInstance::port_value(name: &Port)`).
2. **Make the doc honest:** update the docblock to say `Port` is used by
   the codegen/elaborator layers; the solver works in `AnalogReference`/
   `DigitalNet`. Remove the dead reference to the nonexistent file.

Prefer option 2 (the solver's `AnalogReference`/`DigitalNet` are the right
abstractions for MNA; `Port` is a naming layer).

**Decision rationale.** Dead docs mislead. The `Port` enum has a role
(naming) but it's not the solver's internal abstraction.

**Acceptance criteria.**
- [ ] `port.rs:1` docblock updated; no reference to nonexistent file.
- [ ] `Port`'s actual role documented.

---

### K.6 Consolidate `docs/` — 11 overlapping markdown files

**Spec:** N/A.

**Status:** NOISE. **Medium** (onboarding cost).

**Current state.** `docs/` has 11 `.md` files: `AMS-BUILTIN-TASKS.md`,
`AMS-IR-REFINEMENT.md`, `BNF-AMS.md`, `CLI_TOOLS.md`, `CODEGEN-IR.md`,
`IR-JIT-SPEC.md`, `piperine-hdl-elaboration-phase.md`,
`piperine-hdl-grammar.md`, `piperine-hdl-spec.md`, `SHARED-IR-DESIGN.md`,
`VERILOG_AMS_TECH.md`. Plus `crates/piperine-codegen/IR-SYSTEM.md` and
`AGENTS.md` at root. Significant overlap; unclear which is canonical.

**Proposed solution.**
1. Keep as canonical: `AGENTS.md` (root), `docs/piperine-hdl-spec.md` (the
   spec), `crates/piperine-codegen/IR-SYSTEM.md` (the IR contract),
   `docs/GAPS.md` (this file).
2. Mark the others as superseded: add a one-line header
   `> **Status: superseded.** Canonical reference is now <X>.` and leave
   them for history, or move to `docs/archive/`.
3. `IR-SYSTEM.md` §16 "From IR to solver (current state)" describes the
   *deleted* `ir_expr_to_phdl` round-trip — update to reflect
   `validate_ir_contrib` + `emit_ir_expr`.

**Decision rationale.** Onboarding is blocked by doc ambiguity. Clear
canon + archived history is the standard approach.

**Acceptance criteria.**
- [ ] Canonical docs marked as such.
- [ ] Superseded docs marked or archived.
- [ ] `IR-SYSTEM.md` §16 updated.
- [ ] No dangling references to nonexistent files.

---

### K.7 `synchronise` `tests-baseline.md` and `AGENTS.md` test counts

**Spec:** N/A.

**Status:** STALE. **Low.**

**Current state.** `AGENTS.md` says "~257 tests"; `tests-baseline.md` says
260 (current).

**Proposed solution.** Update `AGENTS.md` to point at `tests-baseline.md`
as the source of truth and quote 260.

**Acceptance criteria.**
- [ ] `AGENTS.md` count matches `tests-baseline.md`.

---

## Part L — Documentation & visibility

### L.1 README rewritten to reflect the actual architecture

**Spec:** N/A.

**Status:** COMPLETELY WRONG. **Critical** (onboarding).

**Current state.** `README.md` describes a Python+ngspice+PyO3 architecture
that does not exist. It lists 9 crates (`piperine-parser`,
`piperine-circuit`, `piperine-ngspice`, `piperine-python`,
`piperine-coordinator`, `piperine-worker`, `piperine-common`,
`piperine-openvaf`, `piperine-interpreter`) — none of which exist. The
`Cargo.toml` has 6 crates (`piperine-ams`, `piperine-cli`,
`piperine-codegen`, `piperine-lang`, `piperine-project`, `piperine-solver`).
Examples use `piperine new my_project` + `piperine run hello.py` (Python
testbenches) — the actual CLI has `check`, `fmt`, `build` (stub), `run`
(stub). References `ARCHITECTURE.md` which does not exist.

**Why it matters.** Any new contributor (human or AI) starts at the README
and is immediately misled. This is the single highest cost/benefit fix in
the whole project.

**Proposed solution.** Rewrite `README.md` to describe the *actual*
architecture from Part 0 of this document:

1. **What Piperine is** (HDL + simulator for analog/mixed-signal; IR-centric;
   in-house solver; optional OSDI).
2. **Architecture diagram** (copy from §0.1).
3. **Crate map** (copy from §0.2).
4. **Quick start** — `cargo build`, `cargo test`, `piperine check
   <file.phdl>`.
5. **Status** — honest "WIP, X% of spec implemented, see `docs/GAPS.md`".
6. **Where to read next** — `AGENTS.md`, `docs/piperine-hdl-spec.md`,
   `crates/piperine-codegen/IR-SYSTEM.md`, `docs/GAPS.md`.

**Decision rationale.** Honesty over aspiration. The README is the front
door; a wrong README is worse than no README.

**Acceptance criteria.**
- [ ] `README.md` rewritten; no reference to Python/ngspice/PyO3.
- [ ] Crate list matches `Cargo.toml`.
- [ ] Examples use real commands.
- [ ] No reference to nonexistent `ARCHITECTURE.md`.

---

### L.2 Module-level `//!` docblocks on every crate's `lib.rs`

**Spec:** N/A.

**Status:** PARTIAL. **Low.**

**Current state.** `piperine-lang/src/lib.rs` has an exemplary docblock
(pipeline diagram, quick start, module table). `piperine-codegen/src/lib.rs`
(23 lines) has none. `piperine-solver/src/lib.rs` (13 lines) has none —
just `pub mod` declarations. `piperine-ams` similar.

**Proposed solution.** Add a `//!` docblock to each crate's `lib.rs`
mirroring the `piperine-lang` pattern: one-paragraph purpose, pipeline
diagram (ASCII), quick-start snippet, module table.

**Decision rationale.** `cargo doc` and IDE hover surface these. The
`piperine-lang` pattern is the template.

**Acceptance criteria.**
- [ ] `piperine-codegen/src/lib.rs` has a `//!` docblock.
- [ ] `piperine-solver/src/lib.rs` has a `//!` docblock.
- [ ] `piperine-ams/src/lib.rs` has a `//!` docblock.
- [ ] `cargo doc` renders cleanly.

---

### L.3 `piperine-solver` re-exports its public API

**Spec:** N/A.

**Status:** PARTIAL. **Low.**

**Current state.** `piperine-solver/src/lib.rs` only has `pub mod`
declarations. Users must import `piperine_solver::circuit::CircuitInstance`,
`piperine_solver::solver::dc::DcSolver`, etc.

**Proposed solution.** Add a prelude or re-exports:

```rust
// piperine-solver/src/lib.rs
pub mod analysis;
pub mod analog;
// ... existing pub mods ...

pub use circuit::CircuitInstance;
pub use device::Device;
pub use solver::dc::DcSolver;
pub use solver::ac::AcSolver;
pub use solver::transient::TransientSolver;
pub use solver::noise::NoiseSolver;
pub use solver::tf::TransferFunctionSolver;
pub use analog::{AnalogReference, Netlist};
pub use digital::{LogicValue, DigitalNet};
```

**Decision rationale.** A flat re-export surface is the ergonomic standard
for library crates. The fully-qualified paths are still available.

**Acceptance criteria.**
- [ ] Common types re-exported at crate root.
- [ ] Existing imports still work (no break).
- [ ] `cargo test -p piperine-solver` green.

---

### L.4 Negative-assertion tests for every silent-bug fix in Part A

**Spec:** N/A.

**Status:** MISSING. **Medium.**

**Current state.** Part A fixes silent bugs. Without a test that asserts
the *old* wrong behavior is gone, regressions can slip back.

**Proposed solution.** For each A.* fix, add a negative-assertion test
mirroring `tests/wave1_nonlinear_tests.rs::power_law_contribution_uses_pow_not_add`
(which asserts `**` is not lowered to `+`). Specifically:

- A.1: assert `I(...)` read in contrib is rejected (not zero).
- A.2/A.3: assert `$temperature`/`$vt` produce non-300K values at `T=350K`.
- A.4: assert `Shl` in digital is rejected (not `Add`).
- A.5: assert `BitNot` in digital is rejected (not `Not`).
- A.6: assert `from_ir` propagates child errors (not silent skip).
- A.7: assert `from_elab` rejects `ddt` (not silent zero).
- A.8: assert unresolved `Param` rejected (not zero).
- A.9: assert unknown terminal rejected (not zero).

**Decision rationale.** Negative assertions catch the specific regression
class (silent fallback returns). Each test names the *old* wrong behavior
and asserts it's gone.

**Acceptance criteria.**
- [ ] One negative-assertion test per A.* fix.
- [ ] Tests pass; `cargo test -p piperine-codegen` green.

---

### L.5 Shared test-device library (reduce fixture duplication)

**Spec:** N/A.

**Status:** NOISE. **Low.**

**Current state.** `Inverter`/`DFF` definitions duplicated across
`tests/digital_topology_tests.rs:25-91`, `tests/cosim_integration.rs:35-82`,
and `src/digital.rs:84-112` (MockInverter). Each test file re-implements
the gates it needs.

**Proposed solution.** Add `crates/piperine-solver/tests/helpers/devices.rs`
with shared `Inverter`, `DFF`, `Nand`, etc. impls. Test files import via
`mod helpers; use helpers::devices::*;`.

**Decision rationale.** DRY. A shared library also makes it easier to add
mixed-signal test devices (Comparator, D2A) for Part E.

**Acceptance criteria.**
- [ ] `tests/helpers/devices.rs` with shared gates.
- [ ] Test files use the shared library.
- [ ] `cargo test -p piperine-solver` green.

---

## Appendix — Build, test, and frozen-file rules

### A.1 Build commands

```sh
cargo build                  # build the workspace
cargo build --release        # release build (LTO, opt 3)
cargo test                   # full suite (~260 tests; see tests-baseline.md)
cargo test -p piperine-codegen   # always re-run after touching codegen
cargo test -p piperine-solver    # solver tests (OSDI subset needs OPENVAF_BIN)
cargo test -p piperine-lang      # PHDL frontend tests
cargo test -p piperine-ams       # AMS frontend tests
```

### A.2 Frozen files (DO NOT EDIT)

Per `AGENTS.md`:

- `crates/piperine-ams/tests/fixtures/**` — frozen VA fixture corpus.
- `crates/piperine-ams/tests/fixtures_fmt/**` — frozen parse corpus
  (despite the name, not a formatter corpus — see G.4).
- `crates/piperine-ams/tests/fixtures_ppr/**` — frozen PPR renditions.
- `crates/piperine-ams/headers/**` — bundled Accellera headers.
- `crates/piperine-solver/tests/va/**` — canonical VA fixtures.

New test fixtures go in **new files** under the appropriate `tests/`
directory. Do not modify existing fixture files.

### A.3 Dependency direction (regression check)

`piperine-solver` does **not** depend on `piperine-codegen`. The codegen
depends on the solver (`Device`, `CircuitInstance`). Verify after any
`Cargo.toml` change:

```sh
cargo metadata --format-version 1 | jq -r '.packages[] | {name, deps: [.dependencies[].name]}'
```

If `piperine-solver` lists `piperine-codegen` as a dependency, the arrow is
broken — revert.

### A.4 Numeric conventions

- Analog values: `f64`.
- Digital values: `LogicValue` (`Zero`, `One`, `X`, `Z` — `#[repr(u8)]`).
- Mixed-signal nets: anonymous `usize` indices (see §6 of the spec, §5 of
  IR-SYSTEM).
- Thermal voltage constant: `k/q = 8.617333262e-5 V/K` (CODATA).
- Default tolerances: `reltol=1e-3`, `vntol=1e-6`, `abstol=1e-12`
  (`src/solver/mod.rs:38-40`).

### A.5 Glossary

- **AMS** — Verilog-A/AMS, the legacy analog HDL (`.va`/`.vams` files).
- **PHDL** — Piperine HDL, the new language (`.phdl`/`.ppr` files).
- **IR** — the shared intermediate representation (`IrProgram`).
- **OSDI** — Open Verilog-A Device Interface; a `.osdi` shared library
  produced by OpenVAF-Reloaded, loaded at runtime by `piperine-solver`.
- **MNA** — Modified Nodal Analysis (the matrix formulation the solver
  uses).
- **BE / Trap** — Backward Euler / Trapezoidal integration.
- **LTE** — Local Truncation Error (timestep-control metric).
- **gmin** — small conductance to ground for matrix regularisation.
- **Capability** — PHDL's name for a trait (`capability Add { ... }`).
- **Bundle** — PHDL's aggregate type (struct-like).
- **Discipline** — PHDL's net type (potential/flow or digital storage).

### A.6 Cross-reference map

| Spec section | IR-SYSTEM section | This doc Part |
|--------------|-------------------|---------------|
| §1 Goals | §1 Overview | Part 0 |
| §4 Items & packages | §2 Program structure | I.10 (pub) |
| §5 Modules | §2 IrModule | F.1, F.2 |
| §5.2 Storage classes (`var`) | §5 Behavior | I.9, I.15 |
| §5.3 Instances | §2 IrInstance | F.3 |
| §5.4-5.5 Arrays, for, if | §5 Loops | F.4, I.11, I.12 |
| §6.1 Value types | §3 Types | B.5, J.4, I.13 |
| §6.2 Disciplines | — | C.1 (Ground) |
| §6.3 Resolution | — | B.4 |
| §6.4 Enums | — | I.7 |
| §6.5 Bundles | §4 BundleLit | I.1, I.6, I.14 |
| §6.6 Capabilities & generics | — | I.2, I.3, I.4, I.5 |
| §7 Functions | §10 Functions | D.5 |
| §7.1 Higher-order | — | I.8, H.7 |
| §8 Behavior | §5 Statements | D.6, D.7 |
| §8.1 Access functions | §4 BranchAccess | A.1, A.9, A.14, A.15, D.9, D.10 |
| §8.2 Analog | §5 Contrib/Force | D.1, D.2, D.3, D.8, A.14 |
| §8.2 (source stepping) | — | H.3 |
| §8.3 Digital | §9 Digital body | D.6, D.7 |
| §8.4 Events | §5 AnalogEvent | E.3, E.4, E.5, J.1 |
| §8.5 Diagnostics | §5 Diagnostic | J.1, J.2, J.3 |
| §9 Phase model | §11 Lowering contract | (enforced by design) |
| §10 No-magic | — | B.2, I.15 |
| §11 Future layers | — | (out of scope) |

**NGSPICE faithful headers:** the gold standard for "what should work."
The 8 model files in `crates/piperine-lang/headers/ngspice/` (see
`docs/NGSPICE_GAPS.md`) exercise every gap listed above. When a gap
is closed, an NGSPICE model that previously failed at elaboration or
codegen should now elaborate and simulate. Test by adding the model
to `crates/piperine-lang/tests/ngspice_compile_tests.rs` (or expanding
`ngspice_parse_tests.rs`).

### A.7 Priority order for implementation

If implementing sequentially, the recommended order (by dependency and
cost/benefit):

1. **L.1 (README)** — 1 day. Highest onboarding ROI.
2. **Part A (silent bugs)** — 3-5 days. Fail-loud everywhere; negative
   tests. **Includes A.14 (`$simparam`), A.15 (`$param_given`)** — both
   needed by every NGSPICE faithful model.
3. **C.1 (Ground) + C.3 (Type/Net)** — 1 day. Unblocks examples.
4. **B.1, B.2 (typecheck width + discipline)** — 3-5 days. Core spec
   promise.
5. **D.1 (forces) + H.4 (MNA branches)** — 1 week. Unblocks VSource/OpAmp;
   prerequisite for A.1 (Option 2 — `I(a,b)` reads as branch-current
   unknown).
6. **D.8 (`$limit` + stateful `limexp`)** — 3-5 days. Inline with D.1 — the
   NGSPICE faithful `diode.phdl`/`bjt.phdl` use `$limit("pnjlim", ...)`.
7. **D.10 (`$analysis`) + D.9 (`ac_stim`)** — 1-2 days. Trivial
   `SimQuery` arms; unblocks `vsrc`/`isrc` DC-vs-tran branching and AC
   analysis.
8. **E.1, E.2 (mixed-signal bridges)** — 1 week. Unblocks SAR/delta-sigma.
9. **D.5 (fn inlining)** — 3-5 days. Unblocks analog functions. Then drop
   `ng_and`/`ng_or`/`sinh`/`cosh`/`tanh` helper fns from
   `ngspice_constants.phdl` (I.11/I.13 let them be inlined).
10. **F.1, F.2, F.3 (from_ir recursion + hierarchy)** — 3-5 days.
    Unblocks Ladder, parent contributions.
11. **G.1 (AMS digital lowering)** — 3-5 days. Unblocks dff.v etc.
12. **I.14 (bundle-typed `param`) + I.15 (mod-body `var`)** — 1 week.
    Unblocks every NGSPICE faithful model's `param model : XxxModel`
    elaboration AND `sw`/`csw` hysteresis state.
13. **I.1, I.3, I.4, I.6 (bundles, Self, generics, BundleLit)** — 1-2
    weeks. Unblocks UInt[N], generic modules.
14. **I.2, I.5 (capability dispatch + conformance)** — 1 week. Unblocks
    operator sugar.
15. **E.4, E.5 (analog events fire + `@ initial` IC/off)** — 3-5 days.
    Prerequisite for the NGSPICE faithful switches E2E.
16. **H.1, H.2 (trapezoidal + LTE)** — 1 week. Solver quality.
17. **D.2, D.3, D.4 (idt, operators, noise)** — 1-2 weeks. Analog surface.
18. **I.11 (`&&`/`||`), I.12 (`else if` in expressions), I.13
    (`sinh`/`cosh`/`tanh`)** — 2-3 days total. Spec ergonomics;
    only matters once everything else works. Can be deferred until
    NGSPICE faithful headers are purged of helper-fn workarounds.
19. **I.7, I.8, I.10 (enums, higher-order, pub)** — 1-2 weeks. Language
    completeness.
20. **H.6 (BJT excess phase PTF), H.7 (breakdown iteration)** —
    model-specific accuracy. Defer until the faithful models run
    end-to-end and only the last 1% accuracy is the gap.
21. **K.* (architecture cleanup)** — 1 week. In parallel with above.
22. **L.2, L.3, L.4, L.5 (docs, re-exports, tests)** — ongoing.

After each step, re-run the `ngspice_compile_tests.rs` suite
(modeled on the existing `ngspice_parse_tests.rs`). The faithful
NGSPICE headers are the primary acceptance test for any gap closure.

---

> **End of `GAPS.md`.** This document is the contract between the spec and
> the implementation. When a gap is closed, strike it through in this file
> and record the close in `tests-baseline.md` with the new test count. When
> a new gap is discovered, add it under the appropriate Part using the same
> template.

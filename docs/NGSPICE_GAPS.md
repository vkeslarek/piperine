# NGSPICE Faithful Models — Gap Analysis

> **Purpose.** This document catalogues every expressiveness gap between the
> NGSPICE model files in `crates/piperine-lang/headers/ngspice/` and the current
> Piperine implementation. The models are written for the *ideal* Piperine —
> they parse correctly (verified by `tests/ngspice_parse_tests.rs`) but many
> constructs cannot yet be elaborated or JIT-compiled.
>
> **Relationship to `GAPS.md`.** Where a gap here overlaps an existing
> `GAPS.md` entry, the cross-reference is given (e.g. "see GAPS A.1"). This
> document adds the NGSPICE-model perspective: *which device equations break*
> and *what the ideal behaviour should be*.
>
> **Conventions.** Same as `GAPS.md`: `file:line` citations are 1-indexed and
> relative to the repo root. Severity levels: **Critical** (silent wrong
> results), **High** (blocks a class of models), **Medium** (parse-only or
> partial), **Low** (polish/ergonomics).

---

## Table of contents

- [Part N — System functions rejected at codegen](#part-n--system-functions-rejected-at-codegen)
  - [N.1 `$simparam` — solver parameters (gmin, step, tfinal)](#n1-simparam--solver-parameters-gmin-step-tfinal)
  - [N.2 `$param_given` — was a parameter explicitly set?](#n2-param_given--was-a-parameter-explicitly-set)
  - [N.3 `$limit` — voltage limiting for Newton convergence](#n3-limit--voltage-limiting-for-newton-convergence)
  - [N.4 `$analysis` — analysis-type branching](#n4-analysis--analysis-type-branching)
- [Part O — Analog operator gaps](#part-o--analog-operator-gaps)
  - [O.1 `I(a, b)` flow reads (see GAPS A.1)](#o1-iab-flow-reads-see-gaps-a1)
  - [O.2 `white_noise` / `flicker_noise` — noise not stamped (see GAPS D.4)](#o2-white_noise--flicker_noise--noise-not-stamped-see-gaps-d4)
  - [O.3 `ac_stim` — AC small-signal stimulus](#o3-ac_stim--ac-small-signal-stimulus)
  - [O.4 `limexp` — not stateful](#o4-limexp--not-stateful)
- [Part P — PHDL grammar gaps](#part-p--phdl-grammar-gaps)
  - [P.1 `&&` / `||` logical operators not in grammar](#p1----logical-operators-not-in-grammar)
  - [P.2 `else if` not supported in if-expressions](#p2-else-if-not-supported-in-if-expressions)
  - [P.3 `sinh` / `cosh` / `tanh` not built-in](#p3-sinh--cosh--tanh-not-built-in)
  - [P.4 Bundle-typed `param` — elaboration untested](#p4-bundle-typed-param--elaboration-untested)
  - [P.5 Module-level `var` as persistent analog state](#p5-module-level-var-as-persistent-analog-state)
- [Part Q — Model-specific gaps](#part-q--model-specific-gaps)
  - [Q.1 BJT excess phase (PTF) — state recurrence](#q1-bjt-excess-phase-ptf--state-recurrence)
  - [Q.2 MOS Meyer capacitances — capacitance vs charge formulation](#q2-mos-meyer-capacitances--capacitance-vs-charge-formulation)
  - [Q.3 Switch hysteresis — `@ above` analog events](#q3-switch-hysteresis---above-analog-events)
  - [Q.4 Diode breakdown voltage — iterative computation](#q4-diode-breakdown-voltage--iterative-computation)
  - [Q.5 `@ initial` for IC / off flag initialization](#q5-initial-for-ic--off-flag-initialization)

---

## Part N — System functions rejected at codegen

> The PHDL lowering (`crates/piperine-lang/src/lowering/expr.rs:422-488`)
> correctly produces `SimQuery` variants for `$simparam`, `$param_given`,
> `$limit`, and `$analysis`. However, the JIT validator
> (`crates/piperine-codegen/src/codegen/ir_emit.rs:526-531`) only accepts
> `Temperature | Vt | Abstime | Mfactor`; everything else hits
> `other => Err(unsupported(...))`.

### N.1 `$simparam` — solver parameters (gmin, step, tfinal)

**Severity:** High.

**Spec:** Verilog-A/AMS standard — `$simparam("gmin", default)` reads a
named simulator parameter, returning `default` if unknown.

**Current state.** The lowering produces `SimQuery::Simparam { key, default }`
(`ir.rs:106-108`). The validator rejects it (`ir_emit.rs:526-531`).

**Affected models.** Every semiconductor device reads `gmin`:
- `dio` — added to junction conductance (`dioload.c:283-298`)
- `bjt` — added to leakage and substrate diodes (`bjtload.c:423-424, 453-454`)
- `jfet` — added to gate diodes (`jfetload.c:245-250`)
- `mos1` — added to bulk junction diodes (`mos1load.c:433-448`)
- `sw` / `csw` — default `roff = 1/gmin` (`swsetup.c`, `cswsetup.c`)
- `vsrc` / `isrc` — default `step` and `tfinal` for waveform params

**Why it matters.** Without `gmin`, junction diodes have zero conductance at
the operating point, causing singular matrices and convergence failure. The
models currently use `$simparam("gmin", 1.0e-12)`; if rejected, the solver
cannot assemble the Jacobian.

**Proposed solution.** Add `Simparam` to the accepted set in `emit_sim`:

```rust
SimQuery::Simparam { key, default } => {
    // Read from sim_ctx — needs a HashMap<String, f64> or fixed fields.
    // For now, hard-code the known keys:
    match key.as_str() {
        "gmin"   => ctx.builder.ins().load(F64, MemFlags::trusted(), ctx.sim_ctx, 24),
        "step"   => /* read from sim_ctx */,
        "tfinal" => /* read from sim_ctx */,
        _ => emit_ir_expr(ctx, default),  // fall back to default
    }
}
```

This requires extending `SimCtx` (`codegen/mod.rs:56-82`) with a
`gmin: f64` field (already present at offset 24 but unused) and `step` /
`tfinal` fields.

**Acceptance criteria.**
- [ ] `$simparam("gmin", 1e-12)` returns the solver's gmin in JIT.
- [ ] `$simparam("step", 1e-6)` and `$simparam("tfinal", 1e-3)` return the
      transient step and final time.
- [ ] Unknown keys fall back to the default argument.

---

### N.2 `$param_given` — was a parameter explicitly set?

**Severity:** High.

**Spec:** Verilog-A/AMS standard — `$param_given("name")` returns 1 if the
named parameter was explicitly set on the instance, 0 otherwise.

**Current state.** The lowering produces `SimQuery::ParamGiven(name)`
(`ir.rs:108-109`). The validator rejects it (`ir_emit.rs:526-531`).

**Affected models.** Used in every device to implement SPICE's
"instance overrides model" parameter resolution:
- `res` — `r`, `w`, `l`, `tc1`, `tc2` override model defaults (`restemp.c:64-91`)
- `cap` — `c`, `w`, `tc1`, `tc2` (`captemp.c:38-60`)
- `ind` — `l`, `nt`, `tc1`, `tc2` (`indtemp.c:38-54`)
- `dio` — `area`, `temp`, `dtemp` (`diotemp.c:77-172`)
- `bjt` — `area`, `areab`, `areac`, `temp` (`bjttemp.c:100-260`)
- `jfet` — `area`, `temp` (`jfettemp.c`)
- `mos1` — `w`, `l`, `ad`, `as`, `temp` (`mos1temp.c`)

**Why it matters.** SPICE parameter resolution depends on knowing whether a
parameter was explicitly set. For example, the resistor's resistance is:
```
if r_given:       use r
elif has_geometry: use rsh * (l - short) / (w - narrow)
elif model.r > 0:  use model.r
else:              use 1 mΩ
```
Without `$param_given`, the models cannot distinguish "user set r=0" from
"r was not given (defaults to 0)".

**Current workaround.** The models use sentinel values: `temp = 0.0` means
"not given, use ambient". This is fragile — 0 K is physically valid in the
math, even if not in practice.

**Proposed solution.** The elaborator already knows which params were given
(it processes the instance's param bindings). Expose this as a bitmask or
`HashMap<String, bool>` on the device's param storage, accessible from the
JIT via a `SimCtx` pointer or a dedicated runtime call.

**Acceptance criteria.**
- [ ] `$param_given("r")` returns 1 when `.r = 50.0` is set on the instance.
- [ ] `$param_given("r")` returns 0 when `.r` is not set.
- [ ] All NGSPICE models resolve parameters correctly.

---

### N.3 `$limit` — voltage limiting for Newton convergence

**Severity:** High.

**Spec:** Verilog-A/AMS standard — `$limit("pnjlim", v, vold, vte, vcrit)`
applies the SPICE `DEVpnjlim` algorithm to limit the per-iteration voltage
change of a pn junction, preventing `exp()` overflow and improving
convergence.

**Current state.** The lowering produces `SimQuery::Limit { kind, args }`
(`ir.rs:110-111`). The validator rejects it (`ir_emit.rs:526-531`).

The `limexp` builtin is available as a partial substitute
(`cranelift_helpers.rs:43-47`: `exp(min(u, 80))`), but it is **not stateful**
— it does not track the previous Newton iteration's voltage, which is the
core of `pnjlim` and `fetlim`.

**Affected models.**
- `dio` — `pnjlim` on `vd` (`dioload.c:180-191`)
- `bjt` — `pnjlim` on `vbe`, `vbc` (`bjtload.c:384-391`)
- `jfet` — `fetlim` on `vgs`, `vgd` (jfetload.c)
- `mos1` — `fetlim` on `vgs`, `vgd` (mos1load.c)

**Why it matters.** Without voltage limiting, the Newton iteration can
diverge when the junction voltage jumps far in a single step (e.g. from
0 V to 0.8 V on a diode). `limexp` prevents overflow but does not guide
the iteration toward the solution. SPICE's `pnjlim` limits the voltage
change to a fraction of `Vcrit`, ensuring quadratic convergence.

**Proposed solution.** `$limit` needs access to the previous Newton
iteration's value (`vold`). This requires a per-device state slot (like
`ddt`/`idt` state). The implementation would:
1. Allocate a state variable for each `$limit` call site.
2. At each Newton iteration, store the current voltage in the state.
3. Apply the limiting algorithm using the stored previous value.

**Acceptance criteria.**
- [ ] `$limit("pnjlim", vd, vd_old, vte, vcrit)` applies the exact SPICE
      `DEVpnjlim` algorithm.
- [ ] `$limit("fetlim", ...)` applies `DEVfetlim`.
- [ ] Diode/BJT/JFET/MOS models converge in the same number of iterations
      as NGSPICE on standard test circuits.

---

### N.4 `$analysis` — analysis-type branching

**Severity:** Medium.

**Spec:** §8 — "Behavior may branch on the current analysis via `$analysis`,
which returns an `Analysis` enum (`Dc`, `Ac`, `Tran`, `Noise`)."

**Current state.** The lowering produces `SimQuery::Analysis(String)`
(`ir.rs:104-105`). The validator rejects it (`ir_emit.rs:526-531`).
`SimCtx` has no `current_analysis` field (`codegen/mod.rs:56-82`).

**Affected models.**
- `vsrc` / `isrc` — DC operating point uses `dc` value; transient uses the
  waveform (`vsrcload.c`: different code paths for `MODEDCOP` vs `MODETRAN`).
- All devices — charge storage is only computed in TRAN/AC, not DC. (Currently
  handled implicitly: `ddt` returns 0 in DC, which is correct. So this gap
  does not break the models — it only prevents explicit branching.)

**Why it matters.** The source models need to switch between DC and transient
values. Currently, the models use `if ($analysis("tran")) { ... } else { ... }`,
which fails at codegen. The `ddt → 0 in DC` behaviour handles the charge
storage case automatically, but the source waveform case requires explicit
branching.

**Proposed solution.** Add a `current_analysis: AnalysisKind` field to
`SimCtx`, set by the solver at the start of each analysis mode. Add
`SimQuery::Analysis` to the accepted set in `emit_sim`.

**Acceptance criteria.**
- [ ] `$analysis("tran")` returns 1 during transient, 0 otherwise.
- [ ] `$analysis("dc")`, `$analysis("ac")`, `$analysis("noise")` work.
- [ ] `vsrc` uses the waveform value in tran and the `dc` value in DCOP.

---

## Part O — Analog operator gaps

### O.1 `I(a, b)` flow reads (see GAPS A.1)

**Severity:** Critical.

**Spec:** §8.1 — `I(a, b)` is a documented branch access function.

**Current state.** Parser and lowering produce
`IrExpr::BranchAccess { access: "I", ... }` (`expr.rs:306-310`). The JIT
emitter returns `f64const(0.0)` for any `access != "V"`
(`ir_emit.rs:90-94`). See GAPS A.1 for full details.

**Affected models.**
- `ind` — `V(p, n) <- l_eff * ddt(I(p, n))` (inductor constitutive relation)
- `mut` — `V(p1, n1) <+ m * ddt(I(p2, n2))` (mutual coupling)
- `ccvs` — `V(p, n) <- trans * I(cp, cn)` (current-controlled voltage source)
- `cccs` — `I(p, n) <+ m * gain * I(cp, cn)` (current-controlled current source)
- `csw` — `var ic : Real = I(cp, cn)` (current-controlled switch)

**Why it matters.** Without `I(a, b)` reads, inductors, mutual inductors,
and all current-controlled devices produce wrong results (inductor: V=0
short without state; CCVS/CCCS: zero output; CSW: never switches).

**Proposed solution.** See GAPS A.1 (Option 2: allocate branch-current
unknowns in the MNA matrix).

**Acceptance criteria.** See GAPS A.1.

---

### O.2 `white_noise` / `flicker_noise` — noise not stamped (see GAPS D.4)

**Severity:** Medium.

**Spec:** Verilog-A/AMS standard — `white_noise(psd)` and
`flicker_noise(psd, exp)` declare noise sources that contribute to the
noise analysis output.

**Current state.** The lowering correctly extracts noise sources into
`IrAnalogBody.noise_sources` (`lowering/expr.rs:49-119`). But
`ir_analog_to_device` never reads `body.noise_sources`, and
`PhdlDevice::noise_current_psd` returns `Vec::new()`
(`runtime/device.rs:279-285`). See GAPS D.4.

**Affected models.** Every device declares noise sources:
- `res` — thermal (`4kT/R`) + flicker (`KF·I^AF/f`)
- `dio` — shot (`2q·Id`) + flicker
- `bjt` — 3 thermal (RC/RB/RE) + 2 shot (Ic/Ib) + 1 flicker
- `jfet` — 2 thermal (RD/RS) + 1 channel + 1 flicker
- `mos1` — 2 thermal (RD/RS) + 1 channel + 1 flicker

**Why it matters.** Noise analysis returns all-zero output for
PHDL-compiled devices. The OSDI path works (for `.osdi` models), so the
solver infrastructure exists — only the PHDL-to-noise stamping is missing.

**Proposed solution.** See GAPS D.4 — wire `body.noise_sources` into
`PhdlDevice::noise_current_psd`, evaluating each `IrNoiseSource`'s PSD
expression at the operating point and returning the vector of (plus_node,
minus_node, psd) triples.

**Acceptance criteria.** See GAPS D.4.

---

### O.3 `ac_stim` — AC small-signal stimulus

**Severity:** Medium.

**Spec:** Verilog-A/AMS standard — `ac_stim(mag, phase)` declares the AC
small-signal stimulus for a source. During AC analysis, the source
contributes `mag · e^{j·phase}` to the RHS.

**Current state.** `IrExpr::AcStim` exists in the IR (`ir.rs`) but is
validated out for analog contributions (`ir_emit.rs:568`). There is no
`$ac_stim` syscall in the PHDL lowering.

**Affected models.**
- `vsrc` — `V(p, n) <+ ac_stim(ac_mag, ac_phase)` (AC voltage stimulus)
- `isrc` — `I(p, n) <+ m * ac_stim(ac_mag, ac_phase)` (AC current stimulus)

**Why it matters.** Without `ac_stim`, AC analysis has no stimulus — the
output is always zero. The solver's AC analysis infrastructure exists; only
the PHDL stimulus declaration is missing.

**Proposed solution.** Two options:
1. Add `$ac_stim(mag, phase)` as a syscall that lowers to `IrExpr::AcStim`.
2. Handle AC stimulus at the solver level: when a `vsrc`/`isrc` has `ac_mag
   > 0`, the solver automatically injects the AC signal during AC analysis,
   without requiring an explicit `ac_stim` call in the model.

Option 2 is simpler and matches how SPICE works (the `.ac` command reads
the `AC` parameter from source lines, not from the device model).

**Acceptance criteria.**
- [ ] AC analysis produces non-zero output when `ac_mag > 0` on a source.
- [ ] AC phase is correctly applied (degrees → radians).

---

### O.4 `limexp` — not stateful

**Severity:** Medium.

**Spec:** Verilog-A/AMS standard — `limexp(x)` is a *limited* exponential
that tracks the previous Newton iteration to prevent divergence. It limits
the *change* in `exp(x)` per iteration, not just clamping `x`.

**Current state.** `limexp` is available as a builtin but is simplified to
`exp(min(u, 80))` (`cranelift_helpers.rs:43-47`). This prevents overflow
but does not implement the stateful limiting behaviour. See also N.3.

**Affected models.** Used in every exponential diode current:
- `dio` — `cdb = csat * (limexp(vd / vte) - 1.0)` (forward region)
- `bjt` — `cbe`, `cbc`, `cben`, `cbcn`, `cdsub` all use `limexp`
- `jfet` — `igs`, `igd` use `limexp`
- `mos1` — `cbd`, `cbs` use `limexp`

**Why it matters.** The simplified `limexp` prevents overflow but does not
guide convergence. The real `limexp` (and `$limit("pnjlim", ...)`) together
ensure quadratic convergence. Without the stateful version, the solver may
need more iterations or fail to converge on stiff circuits.

**Proposed solution.** Implement stateful `limexp` using the same
state-slot mechanism as `ddt`/`idt` (see N.3). The previous iteration's
`exp(x)` value is stored, and the new value is limited to a maximum change
per iteration.

**Acceptance criteria.**
- [ ] `limexp` tracks the previous Newton iteration's value.
- [ ] Diode circuits converge in the same number of iterations as NGSPICE.

---

## Part P — PHDL grammar gaps

### P.1 `&&` / `||` logical operators not in grammar

**Severity:** Low.

**Spec:** §6.1 mentions `Boolean` as a value type and §6.6 lists `Eq` as a
capability, but the spec does not explicitly define `&&` / `||` operators.
The lexer produces `Tok::And` / `Tok::Or` for `&&` / `||`
(`lexer.rs:101-104, 239-242`) but marks them as "not in PHDL grammar."

**Current state.** The binary operator table
(`expr.rs:45-55`) does not include `And` or `Or`. Using `&&` or `||` causes
a parse error: `"Expected RParen, found Some(And)"`.

**Workaround.** The NGSPICE models use helper functions:
```phdl
fn ng_and(a: Boolean, b: Boolean) -> Boolean {
    if (a) { return b; }
    return 0;
}
fn ng_or(a: Boolean, b: Boolean) -> Boolean {
    if (a) { return 1; }
    return b;
}
```

**Affected models.** All semiconductor models use compound conditions:
- `dio` — `ng_and(model.ikf > 0.0, cd > 1.0e-18)`, `ng_or(model.bv >= 1.0e98, ...)`
- `bjt` — `ng_and(oik == 0.0, oikr == 0.0)`, `ng_and(t_tf > 0.0, vbe > 0.0)`
- `mos1` — `ng_and(model.cjsw > 0.0, pd > 0.0)`, etc.

**Proposed solution.** Add `And` and `Or` to the binary operator table at
the lowest precedence (below `BitOr`), or use keyword operators `and` / `or`.

**Acceptance criteria.**
- [ ] `if (a > 0 && b > 0) { ... }` parses and works.
- [ ] `if (a > 0 || b > 0) { ... }` parses and works.

---

### P.2 `else if` not supported in if-expressions

**Severity:** Low.

**Spec:** §8 — if-expressions (`if (cond) { expr } else { expr }`) are
supported. The spec does not mention `else if` chaining for expressions
(only for statements, which the parser does support:
`stmt.rs:220-231`).

**Current state.** In if-expressions (used in `var` initializers and other
expression contexts), `else` must be followed by `{ ... }`, not by `if`.
The expression parser (`expr.rs:94-102`) calls `parse_block()` after `else`,
which expects `{`.

In if-statements (standalone, in behavior blocks), `else if` IS supported
(`stmt.rs:222-224`).

**Workaround.** Replace `else if (B) { Y }` with `else { if (B) { Y } }`.
For chains:
```phdl
// Before (not supported):
if (A) { X } else if (B) { Y } else { Z }

// After (supported):
if (A) { X } else { if (B) { Y } else { Z } }
```

**Affected models.** `passives.phdl` and `mos.phdl` use chained
if-expressions in `var` initializers. All have been converted to the
nested form.

**Proposed solution.** In `parse_primary`'s `if`-expression branch, after
parsing `else`, check if the next token is `if` and parse it as a nested
if-expression (without requiring `{`).

**Acceptance criteria.**
- [ ] `if (A) { X } else if (B) { Y } else { Z }` parses in expression context.

---

### P.3 `sinh` / `cosh` / `tanh` not built-in

**Severity:** Low.

**Spec:** §8.1 — "Built-ins: `ddt(x)`, `idt(x)`, the math functions (`exp`,
`ln`, `sqrt`, `pow`, `tanh`, …)." The spec mentions `tanh` but the
implementation does not include it.

**Current state.** The builtin math table
(`cranelift_helpers.rs:22-51`) includes: `exp`, `ln`/`log`, `log10`,
`sqrt`, `abs`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`,
`pow`, `min`, `max`, `floor`, `ceil`, `limexp`. No `sinh`, `cosh`, `tanh`,
`asinh`, `acosh`, `atanh`.

**Workaround.** Defined as pure `fn`s in `ngspice_constants.phdl`:
```phdl
fn sinh(x: Real) -> Real { return (exp(x) - exp(0.0 - x)) / 2.0; }
fn cosh(x: Real) -> Real { return (exp(x) + exp(0.0 - x)) / 2.0; }
fn tanh(x: Real) -> Real { var e2x = exp(2.0 * x); return (e2x - 1.0) / (e2x + 1.0); }
```

These inline into contributions and are differentiated for the Jacobian.

**Affected models.**
- `bjt` — `tanh` is used in the base resistance IRB formula
  (`bjtload.c:528-531`). Currently unused because the `if (xjrb > 0.0)`
  branch with `tan()` is used instead.

**Proposed solution.** Add `sinh`, `cosh`, `tanh` (and optionally `asinh`,
`acosh`, `atanh`) to the builtin math table with libm wrappers and
symbolic derivatives.

**Acceptance criteria.**
- [ ] `sinh`, `cosh`, `tanh` are available as built-in math functions.
- [ ] Their symbolic derivatives are registered for the Jacobian.

---

### P.4 Bundle-typed `param` — elaboration untested

**Severity:** Medium.

**Spec:** §6.5 — "A bundle is net-capable when every field is a net type;
such a bundle types a port or wire, otherwise a `param`/`var`." A
value-bundle (all value-type fields) should be usable as a `param` type.

**Current state.** The parser accepts `param model : ResModel = ResModel {}`.
The elaboration of bundle-typed params has not been tested. The lowering
needs to:
1. Recognise the bundle type on a `param`.
2. Expand the bundle's fields into individual param slots (or store the
   bundle as a struct).
3. Make field access (`model.rsh`, `model.tc1`) work in analog expressions.

**Affected models.** Every NGSPICE model uses a bundle-typed `param` for
the `.MODEL` parameters:
```phdl
param model : ResModel = ResModel {};
```
Then accesses fields: `model.rsh`, `model.tc1`, `model.tnom`, etc.

**Why it matters.** Without bundle params, the SPICE `.MODEL` / instance
separation cannot be expressed. Every model would need flat params
(`param rsh : Real = 0.0; param tc1 : Real = 0.0; ...`) which is
ergonomically poor and doesn't match the SPICE mental model.

**Proposed solution.** During elaboration, when a `param` has a bundle
type:
1. Look up the bundle definition.
2. Create a param slot for each field, using the bundle's field defaults.
3. When the parent sets `.model = ResModel { .rsh = 50.0 }`, override the
   individual field defaults.
4. In the analog block, `model.rsh` resolves to the corresponding param
   slot.

**Acceptance criteria.**
- [ ] `param model : ResModel = ResModel {}` elaborates.
- [ ] `model.rsh` in an analog block resolves to the field value.
- [ ] `ResModel { .rsh = 50.0 }` overrides only `rsh`, other fields keep
      their defaults.

---

### P.5 Module-level `var` as persistent analog state

**Severity:** Medium.

**Spec:** §5.2 — `var` is "A mutable binding. In a `digital` block it is
combinational unless it must hold a value, when it infers memory." The spec
does not explicitly address `var` persistence in analog blocks.

**Current state.** The switch models (`sw`, `csw`) use a module-level
`var sw_state : Real = 0.0` that is updated inside `@ above(...)` event
blocks and read in the conductance computation. This requires the `var`
to persist across analog evaluations (like a register in digital).

The analog block lowering does not currently support persistent `var`
state. `var` in an analog block is treated as a local that is recomputed
each evaluation.

**Affected models.**
- `sw` — `var sw_state` updated by `@ above(...)` events
- `csw` — same

**Why it matters.** Without persistent state, the switch's hysteresis
machine has no memory of its previous state. The conductance would be
recomputed from scratch each iteration, ignoring the hysteresis band.

**Proposed solution.** Treat a module-level `var` that is assigned inside
an event block (`@ above`, `@ cross`, `@ initial`) as persistent state,
stored in the device's state vector (like `ddt`/`idt` state). The var is
initialised once and updated only when the event fires.

**Acceptance criteria.**
- [ ] Module-level `var` assigned in `@ above(...)` persists across
      analog evaluations.
- [ ] `@ initial { sw_state = 0.0; }` sets the initial value.
- [ ] Switch hysteresis machine works correctly.

---

## Part Q — Model-specific gaps

### Q.1 BJT excess phase (PTF) — state recurrence

**Severity:** Low.

**NGSPICE source.** `bjtload.c:498-519` — the excess phase model uses a
Weil approximation with backward-Euler discretization, implemented via a
state variable `cexbc` and a recurrence:
```
cex(t) = (cc_history + arg3 * cbe(t)/qb) / denom
cexbc[n] = cc + cex/qb
```

**Current state.** The BJT model uses `cex = cbe` (no excess phase). When
`PTF > 0`, the excess phase is silently skipped. The default `PTF = 0`
gives exact parity.

**Why it matters.** Excess phase is rarely used in basic simulations but
matters for high-frequency AC analysis of RF circuits.

**Proposed solution.** The excess phase is a second-order filter on the
collector transport current. It could be modelled as:
1. A pair of `idt` (integration) operators implementing the continuous-time
   transfer function `H(s) = 1 / (1 + s·td/3 + (s·td/3)²)`.
2. Or a dedicated state recurrence operator (like `delay` but for
   second-order filtering).

Option 1 is more PHDL-idiomatic but requires verifying that `idt` can
chain to produce the correct second-order response.

**Acceptance criteria.**
- [ ] When `PTF > 0`, the BJT model applies the excess phase filter.
- [ ] AC analysis matches NGSPICE for a BJT with PTF = 30°.

---

### Q.2 MOS Meyer capacitances — capacitance vs charge formulation

**Severity:** Low.

**NGSPICE source.** `devsup.c:624-689` (`DEVqmeyer`) — returns the
"non-constant half" of Cgs, Cgd, Cgb. The loader
(`mos1load.c:766-786`) averages with the previous half and adds overlap
caps. The charge is updated incrementally: `Q_new = Q_old + C · ΔV`.

**Current state.** The MOS1 model uses the **capacitance formulation**:
```phdl
I(g, sp) <+ cgs_total * ddt(V(g, sp));
```
This computes `C(V) · dV/dt`, which is correct for the small-signal (AC)
response but does not exactly match NGSPICE's incremental charge update.

The **charge formulation** (`ddt(Q(V))`) would be more faithful:
```phdl
var qgs : Real = meyer_qgs(vgs, vgd);
I(g, sp) <+ ddt(qgs);
```
But the Meyer model's charges are not available in closed form for the
linear region (the integral of the Meyer capacitance involves `ln` terms
with singularities at the region boundaries).

**Why it matters.** The capacitance formulation introduces a small
transient charge error that accumulates over time. For most circuits, the
error is negligible. For charge-sensitive circuits (e.g. switched-capacitor
filters), it may cause drift.

The Meyer model itself is inherently non-charge-conservative (it was
designed for computational simplicity). NGSPICE Level 1 uses Meyer; higher
levels (BSIM) use the charge-conservative Ward-Dutton model.

**Proposed solution.** Derive the closed-form Meyer charge expressions for
each region (cutoff, saturation, linear) and use `ddt(Q)`. The
sub-threshold and saturation charges are straightforward; the linear region
requires an integral that involves `ln(2·Vdsat − Vds)`.

**Acceptance criteria.**
- [ ] Meyer charges are expressed as explicit functions of terminal
      voltages.
- [ ] `ddt(Q)` is used instead of `C · ddt(V)`.
- [ ] Transient charge conservation matches NGSPICE on a switched-cap test.

---

### Q.3 Switch hysteresis — `@ above` analog events

**Severity:** Medium.

**NGSPICE source.** `swload.c` — the switch uses a 4-state machine
(REALLY_OFF, REALLY_ON, HYST_OFF, HYST_ON) with explicit state transitions
based on threshold comparisons. On state change, `CKTnoncon++` forces an
extra Newton iteration.

**Current state.** The switch models use `@ above(expr)` analog events to
update the state:
```phdl
@ above(vc - (model.vt + model.vh))  { sw_state = 1.0; }
@ above((model.vt - model.vh) - vc)  { sw_state = 0.0; }
```

This requires:
1. `@ above` to fire when the expression crosses zero (rising).
2. The `sw_state` var to persist (see P.5).
3. The state change to trigger a non-convergence flag (extra Newton
   iteration).

The `@ above` event is in the IR (`IrEventKind::Above`,
`ir.rs:371-386`) but its analog-block semantics (firing during the
Newton iteration, updating persistent state, forcing re-iteration) are not
fully implemented.

**Why it matters.** Without working analog events, the switch state
cannot change during simulation — it stays at its initial value forever.

**Proposed solution.** Implement `@ above(expr)` as an analog event that:
1. Evaluates `expr` at each Newton iteration.
2. When `expr` transitions from negative to positive, fires the event body.
3. The event body updates persistent state (see P.5).
4. The state change sets a flag that forces the solver to re-evaluate
   (like `CKTnoncon++` in NGSPICE).

**Acceptance criteria.**
- [ ] `@ above(expr)` fires when `expr` crosses zero (rising edge).
- [ ] The event body can update persistent `var` state.
- [ ] State changes force an extra Newton iteration.
- [ ] Switch hysteresis matches NGSPICE on a transient sweep.

---

### Q.4 Diode breakdown voltage — iterative computation

**Severity:** Low.

**NGSPICE source.** `diotemp.c:208-244` — the breakdown voltage `tBrkdwnV`
is computed by a fixed-point iteration (up to 25 iterations) that matches
the forward and reverse diode regions at the breakdown point.

**Current state.** The diode model uses the first-order approximation:
```phdl
tBrkdwnV = tBv - model.nbv * vt * ln(1.0 + cbv / tSatCur);
```
This skips the iterative refinement. The approximation is typically within
1% of the iterated value.

**Why it matters.** The breakdown voltage affects the reverse-bias I-V
characteristic near breakdown. The approximation is close enough for most
applications but may cause a small mismatch in the exact breakdown point.

**Proposed solution.** PHDL `fn`s support bounded recursion (§7.1). The
fixed-point iteration could be expressed as a recursive function with a
depth limit:
```phdl
fn ng_bv_iter(xbv: Real, bv: Real, cbv: Real, isat: Real, nbv: Real, 
              vt: Real, tol: Real, iter: Natural) -> Real {
    if (iter >= 25) { return xbv; }
    var xbv_new : Real = bv - nbv * vt * ln(cbv / isat + 1.0 - xbv / vt);
    var xcbv : Real = isat * (exp((bv - xbv_new) / (nbv * vt)) - 1.0 + xbv_new / vt);
    if (abs(xcbv - cbv) <= tol) { return xbv_new; }
    return ng_bv_iter(xbv_new, bv, cbv, isat, nbv, vt, tol, iter + 1);
}
```

**Acceptance criteria.**
- [ ] The iterative breakdown voltage matches NGSPICE's 25-iteration result.
- [ ] Recursion terminates within the depth limit.

---

### Q.5 `@ initial` for IC / off flag initialization

**Severity:** Medium.

**NGSPICE source.** Devices use `MODEINITJCT` and `MODEINITFIX` to
initialize the DC operating point. The `off` flag sets the initial junction
voltage to 0. The `ic` parameter sets the initial condition for transient
analysis (UIC).

**Current state.** The models use `@ initial { ... }` blocks for
initialization:
```phdl
@ initial {
    if (ic != 0.0) { V(pp, n) <- ic; }
}
```

The `@ initial` event is in the IR (`IrEventKind::Initial`) but its
semantics in analog blocks (setting initial voltages, forcing the `off`
state) are not fully implemented.

**Affected models.**
- `cap` — `@ initial { V(p, n) <- ic; }` (initial capacitor voltage)
- `ind` — `@ initial { I(p, n) <- ic; }` (initial inductor current)
- `dio` — `@ initial { V(pp, n) <- ic; }` (initial diode voltage)
- `sw` / `csw` — `@ initial { sw_state = ...; }` (initial switch state)

**Why it matters.** Without `@ initial`, transient analysis with UIC
(User Initial Conditions) cannot set the starting state of energy-storage
elements (capacitor voltages, inductor currents). The `off` flag for DC
operating point convergence is also affected.

**Proposed solution.** Implement `@ initial` as an event that fires once
at `t = 0` before the first Newton iteration. The event body can:
1. Set initial voltages/currents via `V(p,n) <- val` / `I(p,n) <- val`.
2. Set persistent `var` state (see P.5).

**Acceptance criteria.**
- [ ] `@ initial { V(p, n) <- 1.0; }` sets the initial capacitor voltage.
- [ ] `@ initial { sw_state = 0.0; }` sets the initial switch state.
- [ ] Transient analysis with UIC respects initial conditions.

---

## Summary table

| ID | Gap | Severity | Models affected | Workaround |
|----|-----|----------|-----------------|------------|
| N.1 | `$simparam` rejected at codegen | High | all semiconductors, switches, sources | none |
| N.2 | `$param_given` rejected at codegen | High | all devices | sentinel values |
| N.3 | `$limit` rejected at codegen | High | dio, bjt, jfet, mos1 | `limexp` (partial) |
| N.4 | `$analysis` rejected at codegen | Medium | vsrc, isrc | `ddt→0` handles DC implicitly |
| O.1 | `I(a,b)` reads return 0 | Critical | ind, mut, ccvs, cccs, csw | none |
| O.2 | Noise not stamped | Medium | all | none (parsed only) |
| O.3 | `ac_stim` not implemented | Medium | vsrc, isrc | none |
| O.4 | `limexp` not stateful | Medium | all semiconductors | `exp(min(u,80))` |
| P.1 | `&&` / `||` not in grammar | Low | all | `ng_and` / `ng_or` fns |
| P.2 | `else if` in expressions | Low | passives, mos | `else { if ... }` |
| P.3 | `sinh/cosh/tanh` not built-in | Low | bjt | pure `fn` definitions |
| P.4 | Bundle-typed param untested | Medium | all | none |
| P.5 | Module-level `var` persistence | Medium | sw, csw | none |
| Q.1 | BJT excess phase (PTF) | Low | bjt | `cex = cbe` (PTF=0 default) |
| Q.2 | Meyer cap formulation | Low | mos1 | `C·ddt(V)` instead of `ddt(Q)` |
| Q.3 | `@ above` analog events | Medium | sw, csw | none |
| Q.4 | Breakdown voltage iteration | Low | dio | first-order approximation |
| Q.5 | `@ initial` for IC/off | Medium | cap, ind, dio, sw, csw | none |

# Phase 5 — First-class Behavioral Expression Language

Implementation-ready refinement of [ROADMAP.md](ROADMAP.md) Phase 5. Same format as
[PHASE4.md](PHASE4.md): each feature is self-contained — rationale, exact Piperine
surface, the AST / parser / elaboration / device changes, and a test plan. Written
so it can be implemented step by step with no open design decisions.

Headline: a behavioral source's expression becomes a **real Piperine expression**
(`V(a)*V(b) + sin(...)`), type-checked and composable, instead of a raw string.
Most of the machinery already exists — this phase *wires it together*.

---

## 0. Core principle — `$X()` evaluates now, `X()` is an analog expression

This is the semantic split the whole phase rests on:

| Form | Meaning | Where | Who evaluates |
|------|---------|-------|---------------|
| `$X(...)` | **procedural, eval-now** — returns a value immediately | `initial`/`always`/functions | the **interpreter** |
| `X(...)` (bare) | **analog expression** — part of a continuous expression | behavioral params (`.v(...)`) | the **simulator**, every timestep |

So:

```verilog
real f = $sin(1.0);                 // interpreter computes sin(1.0) now → 0.8414…
bsource_v #(.v( sin(6.28*1e3*time) + V(a)*V(b) )) B1(.p(out), .n(gnd));
//            └── bare sin / V(): lowered into the ngspice B-source, evaluated by
//                the simulator at each timestep. NOT computed by the interpreter.
```

- The `$` prefix marks "evaluate in the interpreter, right now, give me the number."
- A **bare** call in a behavioral-expression position is *never* evaluated by the
  interpreter; the whole expression AST is handed to the serializer (§6) and
  lowered to ngspice B-source syntax. Bare `V()`, `I()`, `ddt()`, `idt()`, and the
  math names are **analog primitives** — they only have meaning in this context.

Why this is right: `$sin` and `sin` are genuinely different operations — one is a
host-language number, the other is a node in a continuous-time expression the
solver walks. Marking the procedural one with `$` keeps them unambiguous and means
the same `sin(...)` text reads as "analog" wherever an expression is expected.

> **Doc review needed (flagged):** state this `$X()` vs `X()` rule in
> `docs/lang/expressions.md` and `docs/lang/analog.md`, and note in
> `docs/lang/stdlib.md` that `V()/I()/ddt()/idt()` + bare math are analog-context
> only (procedural code uses `$`-prefixed math). See §8.

---

## 1. First-class behavioral B-source (`bsource_v` / `bsource_i`)  *(headline)*

### 1.1 What the user writes

```verilog
// V = arbitrary expression of node voltages, branch currents, time, math
bsource_v #(.v( V(a)*V(b) )) Bmix(.p(out), .n(gnd));
bsource_v #(.v( V(in) > 0.0 ? V(in) : 0.0 )) Brect(.p(o), .n(gnd));   // half-wave
bsource_i #(.i( gm * V(g,s) )) Bota(.p(d), .n(s));                    // expr + param
```

The `.v(...)` / `.i(...)` argument is a **Piperine expression**, not a string. It is
captured as AST, serialized to ngspice B-source syntax at elaboration, and emitted
as `B<name> p n V={<serialized>}`.

### 1.2 What already exists (do not rebuild)

- `serialize_ngspice_expr(expr, resolver)` in
  `crates/piperine-ngspice/src/expr_serializer.rs` — recursive `Expr` → ngspice
  string. Handles `V()`, `I()`, `ddt()`, `idt()`, math funcs, binops, ternary
  (`Select`), prefix, literals.
- `ParameterValue::Ast(ast::Expr)` in `crates/piperine-circuit/src/types.rs`.
- `resolve_parameters` already stores `ParameterValue::Ast(expr.clone())` for any
  parameter whose definition has `is_expr == true` (i.e. declared `parameter expr`).
- `parameter expr <name>` parses (`ExternParameterKind::Expr` → `is_expr`).

So the expression already arrives at the device's `instantiate` as
`ParameterValue::Ast(expr)`. The only missing wire is: **declare the param as
`expr`, then serialize it in the device.**

### 1.3 Changes

**`crates/piperine-ngspice/ppr/ngspice.ppr`** — change the B-source value param from
`string` to `expr`:

```verilog
extern module bsource_v(inout p, inout n;
    parameter expr V,                 // was: parameter string V
    parameter real temp = 27.0, ... );
extern module bsource_i(inout p, inout n;
    parameter expr I,                 // was: parameter string I
    ... );
```

**`crates/piperine-ngspice/src/hardware.rs`** — `SpiceBSourceV::instantiate` reads the
`V` param as an AST and serializes it. Add an `Element` helper:

```rust
/// `KEY={<serialized expr>}` — a behavioral expression parameter.
fn key_expr(&mut self, key: &str, param: &str, resolver: &dyn NetResolver)
    -> Result<(), ElaborationError>
{
    match self.params.get(param) {
        Some(ParameterValue::Ast(expr)) => {
            let s = crate::expr_serializer::serialize_ngspice_expr(expr, resolver)
                .map_err(|detail| ElaborationError::ConnectionError {
                    instance: self.instance.to_string(), detail })?;
            self.line.push_str(&format!(" {key}={{{s}}}"));   // V={ ... }
            Ok(())
        }
        // tolerate a raw string for back-compat / pre-serialized exprs
        Some(ParameterValue::String(s)) => { self.line.push_str(&format!(" {key}={s}")); Ok(()) }
        _ => Err(ElaborationError::MissingParameter {
            instance: self.instance.to_string(), parameter: param.to_string() }),
    }
}
```

`Element::start` must now also carry the `resolver` (or pass it to `key_expr`),
since `instantiate` already receives `resolver: &dyn NetResolver`. The B-source
bodies become:

```rust
let mut e = Element::start('B', name, p, c, &["p", "n"])?;
e.key_expr("V", "V", resolver)?;     // bsource_v
// e.key_expr("I", "I", resolver)?;  // bsource_i
e.opt("TEMP", "temp", 27.0); …
```

### 1.4 Tests (`tests/e2e_phase5_test.rs`, elaboration-level)

- `bsource_v #(.v( V(a)*V(b) ))` → a spice line containing `V={v(a)*v(b)}`.
- ternary: `.v( V(in) > 0.0 ? V(in) : 0.0 )` → `V={v(in)>0?v(in):0}` (or ngspice
  ternary form the serializer emits).
- `.v( sin(time) + 2.0 )` → `V={sin(time)+2}`.
- a param mixed in: `.i( gm * V(g,s) )` with `gm` a `.param` → resolves/serializes.
- differential `V(g,s)` → `v(g,s)`.

---

## 2. Behavioral E / G (nonlinear VCVS / VCCS)

Same mechanism, different element letter. ngspice nonlinear forms are
`E<n> n+ n- VOL='<expr>'` and `G<n> n+ n- CUR='<expr>'` (or the `B`-style
`V=`/`I=`). Add expression value params:

```verilog
vcvs #(.vol( V(cp,cn) * tanh(V(cp,cn)) )) E1(.p(o), .n(0), .cp(a), .cn(b));
vccs #(.cur( is*(exp(V(cp,cn)/vt) - 1.0) )) G1(.p(c), .n(e), .cp(b), .cn(e));
```

- ngspice.ppr: add `parameter expr vol` to `vcvs`, `parameter expr cur` to `vccs`
  (in addition to the existing linear `gain`/`gm`).
- hardware.rs: if the expr param is present, emit the nonlinear form
  (`E<n> p n cp cn VOL={...}` / `G<n> … CUR={...}`); else the existing linear form.
- Same `key_expr` helper.

## 3. POLY sources

`E1 out 0 POLY(2) (a 0) (b 0) 0 0 0 0 1` — polynomial of controlling inputs. Lower
priority. Provide a `poly(n, controls…, coeffs…)` helper that expands to the POLY
card, or document POLY as expressible via §2 behavioral expressions (preferred —
`E #(.vol( c0 + c1*V(a) + c2*V(b) + … ))` is clearer than POLY ordering). Recommend:
**don't add POLY syntax; lower it to a behavioral expression.** (`NGSPICE_BEHAVIORAL.md §6`.)

## 4. Nonlinear R / C / L (expression-valued passives)

ngspice: `R1 a b R='<expr>'`, `C1 a b Q='<expr>'`, `L1 a b FLUX='<expr>'`.

```verilog
res #(.r( r0*(1.0 + tc*(temp - 27.0)) )) Rt(.p(a), .n(b));   // expression-valued R
cap #(.q( c0*V(a,b) )) Cnl(.p(a), .n(b));                    // charge-defined C
```

- ngspice.ppr: allow the value param to be `expr` as well as `real`. Easiest: add
  optional `parameter expr r` / `parameter expr q` / `parameter expr flux` next to
  the numeric `r`/`c`/`l`; when the expr form is supplied, emit `R={...}` etc.
- hardware.rs: the resistor/cap/ind `value` step checks for the expr param first
  (serialize → `R={...}`), else the numeric `value`. Reuse `key_expr`.

## 5. Behavioral `.func` reuse

A Piperine `function` used **inside a behavioral expression** must be *inlined into
the serialized B-source*, not called procedurally. Two routes; ship the simple one:

- **Inline at serialization (chosen):** when the serializer meets a bare call to a
  known user function, substitute its body with the arguments (the function must be
  pure — only analog primitives/params, no `$`-tasks). Equivalent to ngspice `.func`.
- (Alternative: emit a `.func` card and reference it — more moving parts.)

Keep this minimal; gate it behind the serializer recognizing user-function names.

---

## 6. The serializer — vocabulary and guards

`serialize_ngspice_expr` already maps the analog vocabulary. Phase 5 makes it the
single front door and tightens its guards:

- **Allowed (analog):** `V(n)`, `V(n1,n2)`, `I(branch)`, `ddt(x)`, `idt(x[,ic])`,
  math (`abs sqrt exp ln log sin cos tan asin acos atan sinh cosh tanh pow hypot
  floor ceil min max`), binary/relational/logical ops, ternary `?:`, literals,
  parameters and `temp`/`time`.
- **Rejected (with a clear error):**
  - a `$`-task inside an analog expression (`$op`, `$sin`, …) → "system tasks cannot
    appear in a behavioral expression; use the bare analog form" — *because* `$X`
    means eval-now and that is nonsense in a continuous expression.
  - a bare analog primitive (`V()`, `ddt()`) used in **procedural** code → the
    interpreter already errors ("call to unknown function `V`"); keep that, and make
    the message hint that `V()` is analog-only.
- Verify ngspice operator spellings (`**` for pow, `?:`, `<=`), and that node names
  go through the `NetResolver` so hierarchical nets map correctly.

---

## 7. Parser changes

Essentially none — `parameter expr` and the expression grammar already exist:

| Feature | Parser change |
|---------|---------------|
| `.v( EXPR )` behavioral arg | none — instance params already carry an `Expr` |
| `parameter expr V` | none — `ExternParameterKind::Expr` exists |
| `V()`, `ddt()`, `idt()` in exprs | none — ordinary calls; meaning is contextual |

All work is in `ngspice.ppr` (param kinds), `hardware.rs` (serialize on instantiate),
and `expr_serializer.rs` (guards / function inlining).

---

## 8. Docs to review/update (the flagged change)

The `$X()` vs `X()` distinction touches user docs:

- `docs/lang/expressions.md` — add the rule: `$f(...)` is procedural (eval-now);
  a bare `f(...)` in a behavioral parameter is an analog expression lowered to the
  simulator. Cross-link analog.md.
- `docs/lang/analog.md` — document `V()`, `I()`, `ddt()`, `idt()` as analog
  primitives valid only inside behavioral expressions.
- `docs/lang/stdlib.md` — note the math table is the **procedural** (`$`) set;
  the same names without `$` are the analog set used in behavioral expressions.
- `docs/ngspice/controlled_sources.md` / a new `behavioral.md` — the first-class
  `bsource_v/i`, `vcvs.vol`, `vccs.cur`, nonlinear R/C/L surface.

---

## Implementation order

1. **§1 behavioral B-source** — `parameter expr V/I`, `Element::key_expr`,
   serialize on instantiate. Tests. *(headline; small, unlocks the pattern)*
2. **§6 serializer guards** — reject `$`-tasks in analog exprs, good errors.
3. **§4 nonlinear R/C/L** + **§2 behavioral E/G** — same `key_expr` mechanism.
4. **§5 function inlining**, **§3 POLY-as-expression** — lower priority.
5. **§8 doc updates** — land alongside §1.

Each step is independently shippable and elaboration-testable (no simulator needed
— assert the emitted SPICE line). Done when a testbench can write an arbitrary
analog expression as a device value and see it lowered to a correct ngspice B-source.

# Part V — Builtins Reference

The catalog of what a `.vams` source file may call without a `use`/`include` of any
project code: LRM ch. 4 math functions, LRM ch. 3/§5 analog operators, LRM ch. 9 system
tasks/functions, the Annex D stdlib headers, and the SPICE-faithful device-model
library. This reference is normative for the builtin set as Piperine implements it
today — not every name a resolution table below documents has downstream codegen
wiring, and each gap is called out at its own table entry rather than folded into the
prose.

## §0 How a builtin resolves — no more textual declarations

PHDL-era Piperine resolved every builtin through **MD-24**: a textual `extern fn`/
`extern operator`/`extern task` declaration in a stdlib header, checked by the
elaborator before a Rust-native table was ever consulted, so an unrecognized name was
always a compile-time fail (and so LSP go-to-definition always had a real declaration
line to land on). **That mechanism does not exist for Verilog-AMS.** There is no
`extern` keyword in the VAMS grammar (`crates/piperine-lang/src/keywords.rs` has none),
and the current `headers/` directory carries no `math.vams`/`operators.vams`/
`tasks.vams`/`types.vams` — only `disciplines.vams`, `constants.vams`, and the
`spice/*.va` device-model library (§4).

Builtin resolution today is **native-table-driven, and the table is consulted at
different pipeline stages depending on the kind of name**:

- **`piperine-lang` elaboration does not validate operator/math/system-task call
  names at all.** A `Behavior`'s body is the surface `crate::ast::Stmt`/`Expr` passed
  through unchanged (Part IV §4) — `Expr::Call { name: "ddt", .. }` and
  `Expr::Call { name: "totally_made_up", .. }` are equally well-formed AST nodes to the
  elaborator. Annex C enforcement (`vams_annex_c.rs`, Part I) rejects *constructs*
  outside the analog subset, not *unknown call names*.
- **`piperine-codegen`'s resolve layer is where an unresolved call name fails loud.**
  Math functions resolve against `piperine_lang::math::MATH_FNS` (a native Rust table,
  §1); analog operators resolve against `piperine-codegen`'s own
  `resolve::pom::analog_ops::AnalogOpRegistry` (§2); a name found in neither, and not a
  user-declared `fn`/`analog function`, is `CodegenError::Unsupported` — fail loud, per
  CLAUDE.md's 100% coverage rule, just surfaced one pipeline stage later than PHDL's
  elaboration-time MD-24 check used to.
- **LRM ch. 9 system tasks (`$display`, `$temperature`, …) have no lowering pass at
  all yet** — see §3's gap.

The practical consequence: a typo'd operator or math-function name in a `.vams` source
today is caught at codegen, not at elaboration, and the diagnostic is a generic
`Unsupported` rather than MD-24's old "did-you-mean" elaboration error. There is no
regression guard analogous to the retired `extern_coverage_guard.rs` keeping the two
native tables (`MATH_FNS`, `AnalogOpRegistry`) in sync with any declared surface,
because there is no declared surface for them to drift from.

## Contents

- §1 Math functions
- §2 Analog operators
- §3 LRM ch. 9 system tasks and functions
- §4 Annex D stdlib headers and the SPICE model library
- §5 The digital-subset builtin surface (gates, UDPs) — parsed only
- §6 Piperine's own attribute surface

---

## §1 Math functions

`crates/piperine-lang/src/math.rs::MATH_FNS` — the table of libm intrinsics available
in any expression context, symbolically differentiated by `piperine-codegen`'s
`resolve/diff.rs` for the analog Jacobian. Every entry below is a real, tested table row
(`math_fn(name)` looks it up; `eval_const_math` evaluates it at compile time through the
identical wrapper the JIT links against, so compile-time and runtime results are
bit-identical):

| Fn | Arity | Fn | Arity | Fn | Arity |
|----|-------|----|-------|----|-------|
| `sin` | 1 | `sinh` | 1 | `exp` | 1 |
| `cos` | 1 | `cosh` | 1 | `limexp` | 1 (exponent clamped to 80, Newton robustness) |
| `tan` | 1 | `tanh` | 1 | `ln` | 1 |
| `asin` | 1 | `asinh` | 1 | `log10` | 1 |
| `acos` | 1 | `acosh` | 1 | `sqrt` | 1 |
| `atan` | 1 | `atanh` | 1 | `pow` | 2 |
| `atan2` | 2 | `hypot` | 2 | `abs` | 1 |
| `min` | 2 | `max` | 2 | `floor` / `ceil` | 1 |

`log` is accepted as an alias for `ln` (`math_fn` special-cases it before the table
lookup) — the LRM's own ch. 4 `ln` naming is canonical; `log` exists for source
compatibility with real Verilog-A corpora.

---

## §2 Analog operators

`crates/piperine-codegen/src/resolve/pom/analog_ops.rs::AnalogOpRegistry` — every
operator lowers to a POM `Expr` marker call (`__<op>(state_id, resolved_args...)`) that
the flattener and Cranelift `Builder` dispatch on by name:

| Operator | Registered name(s) | Notes |
|---|---|---|
| `ddt` | `ddt` | time derivative, `Q(V)` companion model |
| `idt` | `idt` | time integral; AC admittance `1/jω` (Part X) |
| `idtmod` | `idtmod` | modulo-wrapped integral |
| `ddx` | `ddx` | partial derivative w.r.t. a branch quantity |
| `absdelay` | `delay`, `absdelay` | both spellings dispatch the same operator |
| `transition` | `transition` | piecewise-smoothed step |
| `slew` | `slew` | rate-limited follower |
| `table` | `table` | 1-D interpolation (`piperine-signal::behavior::table`, Part IX) |
| `laplace_np`/`_zp`/`_pm`/`_nm`/`_npm` | `laplace_{np,zp,pm,nm,npm}` | s-domain transfer function, all five LRM variants registered |
| `zi_zd`/`_zp`/`_nd`/`_np` | `zi_{zd,zp,nd,np}` | z-domain transfer function, all four LRM variants registered |
| `ac_stim` | `ac_stim` | AC-only stimulus contribution; phasor-summed across multiple calls on one contribution (`flatten/analog.rs::split_ac_stim`) |
| `white_noise`/`flicker_noise` | `white_noise`, `flicker_noise` | noise-current PSD sources |

**Correction to `ROADMAP.md`'s "P1 backlog" table and to CLAUDE.md's "Known gaps"
section, both stale relative to the source on this branch:** both currently claim
`laplace_*`/`zi_*` are "not declared at all" and fail loud at elaboration. That was true
of an earlier state of the codebase; **the registry above is live and populated with
all nine LRM variants** (`analog_ops.rs:393-399`, `AnalogOpRegistry::with_builtins`).
`laplace_*`/`zi_*` calls reach `piperine-codegen` unresolved (§0 — VAMS elaboration
performs no operator-name check) and resolve successfully through this registry, the
same as `ddt`/`idt`. This spec text is grounded directly in the registry's own source,
not in `ROADMAP.md`'s prose, precisely because the two disagree.

Event-detector operators (`cross`, `above`, `timer` — LRM §5.10.5) are a distinct
mechanism from the `Expr::Call`-shaped operators above; they are recognized by
`crate::ast::EventSpec`'s own named-event grammar (parsed directly, not resolved as a
call) and lowered by codegen's event machinery (Part X, Part IX §`events.rs`).

---

## §3 LRM ch. 9 system tasks and functions

`crates/piperine-lang/src/eval/tasks.rs::SystemTasks` implements the ch. 9 (and ch. 17
diagnostic/ch. 9.13 conversion) surface as a **pure, directly-testable name table and
semantics module** — `$write`, `$strobe`, `$display`, `$monitor` (§9.2 format-string
family, via `format_display`), `$info`/`$warning`/`$error`/`$fatal` (§17.1 severity
tasks), `$finish`/`$stop` (§9.5.1), `$discontinuity`/`$bound_step` (§9.5.2/3, VAMS
analog-specific), `$abstime`/`$temperature` (§17.2, via `SimEnvironment`),
`$itor`/`$rtoi`/`$realtobits`/`$bitstoreal`/`$signed`/`$unsigned` (§9.13 conversions).
Unknown names fail loud through `SystemTasks::resolve` with Levenshtein-distance-3
"did you mean" suggestions.

> **This module's own doc comment discloses the real gap, and it is still open on this
> branch:** `crate::ast::Stmt::TaskCall`/`Expr::SystemFunctionCall` parse correctly, but
> **nothing in `piperine-lang`'s elaboration pipeline lowers a `$`-task call into the
> POM**, and there is no VAMS-shaped runtime interpreter to invoke `SystemTasks`
> against — the PHDL-era `Interpreter`/`Host` trait died with `parse/` (MD-45) and was
> never rebuilt for VAMS. Writing `$display("%f", V(a,b));` inside an analog block
> parses and elaborates today, but the call currently has nowhere to go before it
> would need to reach a compiled kernel — it is dead weight in the AST once elaboration
> hands the behavior body to codegen, not a working diagnostic print. T47's own
> Progress Log entry names this precisely as a SPEC_DEVIATION: this file delivers the
> *semantics* ch. 9 needs, "so that whichever pass eventually lowers `TaskCall`/
> `SystemFunctionCall`... has a ready, independently-verified implementation to call
> into" — that lowering pass does not exist yet.

`VpiTaskKind`/`VpiTaskRegistry` (T53, ch. 11's VPI-associated `$name` mechanism) is a
distinct, later addition in the same file: it models how a `$name` becomes callable
system-task/function surface beyond the closed ch. 9 table, routed through Piperine's
own native plugin ABI rather than a `dlopen`'d IEEE-VPI C shim (VPI hosting is
explicitly out of scope — see ROADMAP.md and Part VI). It has the same "semantics
present, no live dispatch wiring" character as the rest of this file.

---

## §4 Annex D stdlib headers and the SPICE model library

`crates/piperine-lang/headers/`:

- **`disciplines.vams`** — LRM Annex D.1, migrated by hand from the LRM's own listing
  (T40, VAMS-09), byte-for-byte the standard package every VAMS implementation ships:
  `logic`/`ddiscrete` disciplines, the full `Current`/`Charge`/`Voltage`/`Flux`/...
  nature/discipline set with their `_ABSTOL`-overridable tolerances.
- **`constants.vams`** — LRM Annex D.2, same migration provenance: `M_*` math constants
  and `P_*` physical constants (`P_Q`, `P_K`, `P_H`, `P_EPS0`, `P_U0`, `P_CELSIUS0`),
  each with SPICE/old/NIST1998/NIST2010 variants selectable via `` `define ``, defaulting
  to NIST2010 (unchanged numeric values from the PHDL-era `constants.phdl` it replaced).
- **`spice/*.va`** — the ngspice-faithful device-model library (`diode.va`, `bjt.va`,
  `mos.va`, `jfet.va`, `switches.va`, `urc.va`, `tline.va`, `controlled.va`,
  `sources.va`, `passives.va`, `constants.va`, `lib.va`), reached via
  `use spice::<file>;` in any project — no external dependency, the models are the
  numerically-validated corpus `tests/ngspice_*.rs` cross-checks against real ngspice
  (13 models, 6 analysis domains, `tests/ngspice_parity_matrix.rs` is the coverage
  gate). A project package literally named `spice` shadows the builtin.

There is no `types.vams`/`operators.vams`/`math.vams`/`tasks.vams`/`statistics.vams`/
`constraints.vams`/`device_port.vams` — those PHDL-era MD-24 declaration headers are
gone along with the `extern` mechanism itself (§0). The dv-core `tol`-clause
statistical-distribution surface (`Distribution::kind`, Part IV §4) and the dv-core
constraint-helper surface both resolved against headers in that deleted set and have no
VAMS Annex A production to replace them — they are dark, not merely undocumented
(Part IV's `ConstraintBlock` removal note applies to the same underlying gap).

---

## §5 The digital-subset builtin surface (gates, UDPs) — parsed only

LRM ch. 7's gate-primitive and user-defined-primitive (UDP) surface is **parsed and its
semantics are independently implemented and tested, but not wired into the live
digital-device pipeline**:

- `GatePrimitive::eval`/`eval_n` (`crates/piperine-codegen/src/kernel/digital/gates.rs`,
  T48) — the full §7.2/§7.3/§7.4 unidirectional truth-table family:
  `and`/`nand`/`or`/`nor`/`xor`/`xnor` (Table 7-4), `buf`/`not` (Table 7-9),
  `bufif0`/`bufif1`/`notif0`/`notif1` (Table 7-10), plus `Strength`/`Strength::resolve`
  (§7.9 Table 7-3 drive-strength combination). The switch-level primitives
  (`nmos`/`pmos`/`cmos`, `tran*`, `pullup`/`pulldown`) are out of scope even at the
  pure-function level — they need a bidirectional resistive network model, not a truth
  table, and no work has started on them.
- `UdpTable` (`crates/piperine-codegen/src/kernel/digital/udp.rs`, T49) — Annex A.5.3 /
  §7.4.3-7.4.6 combinational and sequential state-table evaluation against an
  already-parsed `PrimitiveDecl`.

Both files' own doc comments state the identical gap plainly: `piperine-lang`'s
elaboration pipeline does not lower a `GateInstantiation` or `PrimitiveDecl` instance
into the POM at all, and nothing in `piperine-codegen` reads them — the fused
digital-network JIT compiles `always`/`initial` block semantics (`LoweredBody`), never
raw gate/UDP instances. Instantiating `and g1(y, a, b);` in a `.vams` source parses, but
there is no path from that instance to a simulated device today. `RegionQueue`
(`crates/piperine-solver/src/digital/regions.rs`, T51 — LRM §8.3's four-region
stratified event cycle) has the same character: a standalone, independently-verified
ordering primitive, not wired into `DigitalState`'s live scheduler
(`scheduler.rs::evaluate_dag_ordered`/`evaluate_until_stable`), which still only
implements a two-phase (register-commit, then comb re-evaluation) delta cycle.

---

## §6 Piperine's own attribute surface

The `device`/`port` attribute schemas (`(* device = "...", type = "..." *)`,
`(* port = "...", kind = "..." *)`) are Piperine's one extension over standard Annex A
syntax — covered precisely in Appendix B §B.2 and Part I §8, not restated here.

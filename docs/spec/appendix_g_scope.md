# Appendix G. Scope and open questions

This appendix lists what the language leaves out on purpose (G.0) and every question this specification leaves open (G.1 onward).
An open item is not resolved here. Where a rule is in force while the question stands, the entry says which.

Item numbers are stable. Numbers missing from the sequence belong to closed items (see "Closed items").

## Status values

| Status | Meaning |
| --- | --- |
| **Open** | undecided; the specification states no rule, or states two that are not reconciled |
| **Provisional** | a rule is in force and normative; the item asks whether to keep it |
| **Planned** | a feature outside this specification, intended for a later one |
| **Implementation** | the language is defined; the reference implementation does not yet meet it (Appendix C.2) |
| **Flagged** | the design was silent; a part resolved it locally with a `Resolution` note, listed here for review |
| **Resolved provisionally** | a `Resolution` note narrows or extends what the design states; the note is normative, and the item asks for confirmation |
| **Closed** | decided; the entry cites the deciding section |

## G.0 Out of scope

These are excluded by design. Each has no construct in the language (Appendix B.1).

| Excluded | Use instead |
| --- | --- |
| formal-verification constructs | monitors (§22.5) and the host (§23) |
| the SystemVerilog verification language | a bench in MSL for stimulus and checks, the host for sweeps, Monte Carlo and regression (§23.1) |
| blocking writes and `#0` | commits; every read sees the past (§12.1.1) |
| indirect contributions | the row of an internal node (§11, `Nullor`) |
| heap types | arrays and carriers fixed at elaboration (§1 rule 2) |
| references, borrows, moves, mutability markers | values, copied where they are used (§1.4.6); hardware named by identity (§1.4.7); G.71 |
| reading files during a run | `include_str!` into a `const` (§20) |
| implicit converters | `D2A`, `A2D`, `Bidir` placed explicitly (§6.1, §22.4) |
| global nets other than `gnd` | ports (§6 rule 1) |
| physical units in the type system | units are labels (§4 rule 3) |
| charge-storage strengths on nets (`trireg` sizes) | an analog model of the charge |
| macros and format strings | functions over `emit!`; typed record fields (§19, §22.6) |
| named parallel branches | an internal node (§6 rule 3) |
| optional ports | a second module for the variant |
| inheritance between natures | a second nature (§4 rule 5) |
| lints as part of the language | a tool may warn about anything; no rule depends on a warning |
| `uic` | a held node gives the same start, solved (§16) |
| an end-of-analysis event | the host keeps the last record (§22.6.3) |
| source search paths (`-I`) | a dependency named in the manifest (§3.6 rule 12) |
| a package registry | `path` and `git` dependencies (§3.6 rule 7) |

**Highest impact:** G.6 (the implementation gaps), G.8 (import), G.21 (lossy lines), G.22 (load-dependent delay).

## Summary

| Item | Title | Status |
| --- | --- | --- |
| G.1 | Name | Open |
| G.4 | Bundles with extra ports | Open |
| G.5 | Result syntax | Provisional |
| G.6 | AIR and Piperine gaps | Implementation |
| G.8 | Import | Planned |
| G.9 | Real-valued change | Provisional |
| G.10 | Time resolution | Open |
| G.11 | Custom floats | Open |
| G.13 | Metastability | Open |
| G.14 | Field-init shorthand | Closed |
| G.15 | Keywords as names | Closed |
| G.16 | Shadowed probes | Open |
| G.17 | The top module | Provisional |
| G.19 | Depth of history | Provisional |
| G.20 | Reductions | Open |
| G.21 | Lossy lines and measured responses | Open |
| G.22 | Load-dependent delay | Open |
| G.23 | Bit fields on values | Open |
| G.24 | Frequency-domain post-processing | Provisional |
| G.30 | Small-signal response of sampled blocks | Implementation |
| G.32 | Delayed logic where time does not advance | Provisional |
| G.33 | `analysis(Static)` in an analysis without time at a transient's last instant | Open |
| G.34 | Commits made during such an analysis | Open |
| G.35 | Continuing after `finish` | Open |
| G.36 | Flags of a periodic analysis | Open |
| G.37 | Periodic small-signal: no traced example, no implementation | Open |
| G.38 | Nothing has been compiled | Closed |
| G.39 | Size of the addition | Open |
| G.40–G.44, D1–D17 | Closed in this specification (one table; D4 is G.38) | Closed |
| G.45–G.48 | Flagged resolutions (one table) | Flagged |
| G.49–G.63 | Resolutions that narrow or extend the design (one table) | Resolved provisionally |
| G.64 | Implementation-defined numerics: spline end conditions; `normal` is fixed | Resolved provisionally |
| G.65 | The `"cosim"` interface | Closed |
| G.66 | Session parameter names of `simparam` | Open |
| G.67 | Two blanket traits with one method name | Open |
| G.68 | `Tran` passed to `analysis` names a module | Open |
| G.69 | Implementation-defined float results: the bits of a NaN, the last place of the elementary functions | Resolved provisionally |
| G.70 | The rules of the value language that were formerly taken from Rust | Resolved provisionally |
| G.71 | Value semantics: no reference, no move, no `mut`; the implementation and `std` still use the earlier forms | Implementation |
| G.72 | Explicit unknowns: no value carries a flag; `Logic`, `Option` and `Default` say what a type may hold; the implementation still carries the flag | Implementation |

## Open items

### G.1 Name

**Status: Open.**
The language needs a name distinct from Piperine (the toolchain) and AIR (the IR). This specification calls it "the language".

### G.4 Bundles with extra ports

**Status: Open.**
A real DRAM cell also needs a plate voltage. `Fn(Cell)` (§8.4) takes one port list.

| Option | Effect |
| --- | --- |
| put the plate in the bundle `Cell` | no language change; every cell variant shares the field |
| give `Fn` a second argument | the bound names two port groups |

### G.5 Result syntax

**Status: Provisional.** In force: a module with a result ends its body with the result expression (§8.3.2).
Alternatives under consideration: a named result port, or `-> name: T`.

### G.6 AIR and Piperine gaps

**Status: Implementation.** The language is defined; the reference implementation does not yet provide:

| Gap | Defined in |
| --- | --- |
| switch branches | §11 rule 2 |
| the explicit model of unknowns | G.72 |
| resolved natures | §4, §5.2 |
| user `limit` functions | §18 |
| integer time | §15 |
| value inputs and results | §8.3 |
| the periodic small-signal linearization | §16.3 |

Appendix C marks each affected row.

### G.8 Import

**Status: Planned.**
Running Yosys netlists needs its own part: the netlist form consumed (RTLIL or cell netlists) and a cell-to-language table.

| Cell | Maps to |
| --- | --- |
| `$mux` | `if` |
| `$tribuf` | `Tribuf` on a `Wire` (§22.3.5) |
| `$dff` | a reaction |
| `inout` | a `Wire` port |

Several writers in a behavioral netlist are several drives of a resolved carrier (§5.2).

### G.9 Real-valued change

**Status: Provisional.** In force: a `Bit<f64>` changes when its levels change. `0.0 → -0.0` is a change; the same `NaN` is not.
The rule follows from §5.3 applied to the `Repr` of `f64`. The item asks whether numeric equality should be used instead.

### G.10 Time resolution

**Status: Open.**
The resolution is one session option (§15). Whether one design may mix resolutions is undecided. In force: one resolution per session.

### G.11 Custom floats

**Status: Open.**
`Float<E, M>` for bfloat16 and 8-bit formats. In force: `Float<N>` with `N` in {16, 32, 64, 128} (§1).

### G.13 Metastability

**Status: Open.**
A stdlib synchronizer whose first stage commits X (a `Logic`) when `since(d) < tw` at the edge. It needs no language change (§22.5.1 rule 4 shows the pattern).

### G.14 Field-init shorthand

**Status: Closed.**
`{ r }` for `{ r: r }` in instances and templates is accepted, as in a struct expression (§1.3.3), and the tree keeps the authored form (Appendix B.3).

### G.15 Keywords as names

**Status: Closed.**
Raw identifiers are accepted for the seven added keywords and for no other word (`r#out`), listed without `r#` in metadata (Appendix A.1).

### G.16 Shadowed probes

**Status: Open.**
Whether a tool should warn when a local takes the name of a probe in scope. No rule depends on the warning (G.0).

### G.17 The top module

**Status: Provisional.** In force: the choice is the host's. It is any module without ports, or one the host names (§23.1).
The item asks whether the language should mark a top module.

### G.19 Depth of history

**Status: Provisional.** In force: one previous value and one time (§5.3, §22.1.8). Longer patterns are state.
The item asks for confirmation that this is enough.

### G.20 Reductions

**Status: Open.**
An explicit signal per reduction (§19) is plain but verbose. Stdlib modules with a result, such as `running_max(x)`, would shorten it. None is defined in §22.

### G.21 Lossy lines and measured responses

**Status: Open.**
Lossy and coupled lines (SPICE `O`, `Y`, `P`) and S-parameter blocks need a convolution over the input's history.

| Option | Effect |
| --- | --- |
| rational fitting into `laplace_zp` (§22.2.14) | stdlib only; accuracy is that of the fit |
| a core `convolve(x, h, tmax)` | one more core primitive; changes the count of Appendix B.1 |

### G.22 Load-dependent delay

**Status: Open.**
A gate's delay often depends on the load it drives (XSPICE input loads, Liberty NLDM tables). A module cannot see what its output connects to.
On a resolved carrier the load can be a field of the drive type that `resolve` sums. Whether that is enough is to be confirmed before an elaboration query is added.

### G.23 Bit fields on values

**Status: Open.**
Slices and concatenations of integers (Yosys `$slice`, `$concat`) are written with shifts, masks or `cast` (§22.1.3). A stdlib `bits::<H, L>(x)` and a concatenation function would read better. A `concat!` form would be a third built-in `!` form (Appendix A.1).

### G.24 Frequency-domain post-processing

**Status: Provisional.** In force: measures over complex results belong to the host. A sweep over frequency has no instants, so no reaction runs in it (§16.2).
The item asks whether to confirm this or add a hook.

### G.30 Small-signal response of sampled blocks

**Status: Implementation.**
Around an operating point a sampled block has a response only if it states one, as `zi_nd` does (§11.1, §22.2.12).
A general one, such as a digital loop filter between an ADC and a DAC, gets it from the periodic small-signal analysis (§16.3), as far as the values it carries are real (§23.3.4).
That analysis is defined and not yet implemented (Appendix C.2, G.37).

### G.32 Delayed logic where time does not advance

**Status: Provisional.** In force: a drive written with a bare `after` holds its value in `Dc` (§16.1.3), as in VAMS. A DC sweep does not move it.
Logic that must follow a sweep says so, in one of two ways:

```rust,fragment
q = e after if analysis(Transient) { tpd } else { 0.0 };     // §8.2.5 allows it
q = inertial(e, tpd);                                    // follows its input in an operating point (§22.5.8)
```

### G.33 `analysis(Static)` in an analysis without time at a transient's last instant

**Status: Open.**
A `Dc`, `Ac` or `Noise` analysis run at a transient's last instant is one more instant of that transient (§16.4.2). It has equilibrium solves, so `analysis(Static)` is true there (§11.1).
Reactions guarded by `analysis(Static)` therefore run. §16.4 does not say whether they should.

| Item affected | What happens under the rules as written |
| --- | --- |
| `zi_nd` (§22.2.12) | `on settle && analysis(Static)` runs and commits a steady-state history over the sampled one |
| `idt` with `ic` (§22.2.8) | held at `ic` in those solves, not at its integrated value |
| `transition`, `inertial` (§22.2.11, §22.5.8) | follow their input directly if it changes in that instant |
| `CapIc` (§16) | unaffected in `Dc`: its guard also requires `analysis(Transient)` |

What is to be decided: whether `Static` is true there, and if so whether seeding reactions may overwrite state the transient built.
G.34 decides whether those commits survive.

### G.34 Commits made during an analysis without time at a transient's last instant

**Status: Open.** Two rules apply and are not reconciled.

| Rule | Says |
| --- | --- |
| §16.1.3 | a commit with `after d`, `d > 0`, never comes due and is dropped when the analysis ends |
| §16.4 | an analysis run at a transient's last instant keeps the event queue, and the transient can be continued after it |

The question covers two kinds of commit made during that analysis:

1. A delayed commit, `q <- x after d`. Either it is dropped with the analysis, or it stays in the queue and comes due in the continued transient.
2. A `settle` commit. It is due at microstep 0 of the next time point (§15). Either that is the first time point of the continued transient, or the commit is dropped as at a stop time.

Commits that were pending before the analysis began are not in question: the queue stays (§16.4.2).

### G.35 Continuing after `finish`

**Status: Open.**
A record with `finish = true` on `sim` ends the analysis at its instant, which is the last time point (§19 rule 5, §22.6.2).
§16.4 does not say whether a transient ended this way can be continued, nor whether one ended by `fatal` can.

### G.36 Flags of a periodic analysis

**Status: Open.**
The table of §11.1 says `Transient`, `Ac` and `Noise` are constant during an analysis. A periodic small-signal analysis is `Transient` for its trajectory and `Ac` or `Noise` for its linearization (§16.3).
The exception is stated only in the "the model sees" column of §16.3. The table of §11.1 is not amended.

### G.37 Periodic small-signal: no traced example, no implementation

**Status: Open.**
The periodic small-signal and noise analyses are defined by §11.1, §16.3 and §23.3.4. No gallery model traces one, and the reference implementation has none (Appendix C.2, G.6).
A traced example is wanted so that the definition is exercised. `SampleHold` (§12.1) under a periodic small-signal analysis is the candidate: it must show its zero-order-hold response.

### G.38 Nothing has been compiled

**Status: Closed.**
The standard library is source: `crates/piperine-lang/std/*.msl` compile with zero diagnostics in the POM, and Part VII's fences are excerpts of them (§22). Two guards keep that true. `std_excerpts` ties each Part VII fence to its `std` file and each `std` item to a fence. `std_galleries` builds the 18 gallery fences of Appendices D, E and F against the embedded library and requires no diagnostic, a sound design and no unresolved member.
What the gates prove is a front end: the library and the galleries are parsed and resolved, and nothing is type-checked, elaborated or executed. The conformance set for the first compiler pass is in Appendix C.2.3: `transition`, `zi_nd`, `inertial`, relaxation of `Tran` on `Wire`s, and the trace of §15.1.

### G.39 Size of the addition

**Status: Open.** No action is planned.
The language has 62 mixed-signal constructs beside its value language (Appendix B.1), for 11 concepts (§0). No further construct has been found that can be removed without a model paying for it.

## Flagged resolutions

These were silent in the design. A part resolved each with a `Resolution` note, which is normative. They are listed for review because each affects portability between implementations.

| Item | Resolved in | In force | For review |
| --- | --- | --- | --- |
| G.47 Symbol lookup for `extern "C" fn` | §3 rule 2 and its notes | the toolchain locates the symbol; `#[link]` is defined on extern modules only (§8.5) | whether a model should name the library |
| G.48 `pub` items of an instance bound to its result | §8.2.14 | a body `let` bound to a module with a result names the result (§8.3.3); the instance's `pub` items are not reachable as `name.item` | whether such an instance needs a second name, which bears on G.5 |

## Resolutions that narrow or extend the design

Each item is in force through the `Resolution` note cited, which refers back here. Each is listed because it changes what the design states, not only what it leaves silent.

| Item | Resolved in | In force | Against the design |
| --- | --- | --- | --- |
| G.49 | §4 rule 13 | compatible continuous natures also need equal flow names when both declare `flow` | §4 rule 5 tests potential names only; a `Servo` node (§11) no longer meets an `Electrical` one |
| G.50 | §13 rule 11 | the argument of `at` is a discrete expression: no probe, `now()`, `freq()`, `gmin()`, `source_scale()` or `analysis(Static)` | the design says only that `now()` is not an input |
| G.51 | §8.2.21 | an undelayed definition forbids an initial value on the levels it defines | a new static error; no model of this specification is hit |
| G.52 | §13 rule 16, §5.3.11 | `p.changed()` never fires at `(0, 0)`; a `set` made before the run binds the param | the design commits such a `set` at `(0, 0)` like an initial value |
| G.53 | §8.4.4 | a placement through a template value takes no param block | removes `cell(c) { m: 2.0 }` |
| G.54 | §8.5.3 | `m` and `dtemp` are readable names inside a module, with their effective values | the design has them as implicit params only |
| G.56 | §8.3.6 | every core result has a direct dependence on its value inputs | `let y = delay(f(y), td);` is an error |
| G.58 | §11 rule 3 | the selector rule also binds the flow contributions of a potential-capable branch; a probed short is not merged | extends the rule, with one exception clause |
| G.59 | §6 rule 5 | `p.inflow()` includes the branches of child instances that end on `p` | the design says "this module's branches" |
| G.60 | §15 T2, §15 T3 | a half-tick tie rounds to the later tick; a positive delay that rounds to no advance is the next microstep | observable at half-tick delays |
| G.61 | A.1, §1 | an unlisted attribute is an error; `char` and every lifetime are dropped | closes the token classes and the attribute set (A.1, A.8) |
| G.62 | §1 | `usize` and `isize` are 64 bits on every platform | a fixed width, whatever the machine; needed for §0.1.1 |
| G.63 | §23.1 | a `force` on a node is a potential source to `gnd`; a host action answering an event is due at `(t, n + 1)` | new host semantics |

### G.64 Implementation-defined numerics

**Status: Resolved provisionally.** In force: the generator of `normal` is the one written in `std::analog` (§22.2.17): SplitMix64 for two uniform draws and Box-Muller for the sample. The end conditions of the splines of `table_model` (§22.2.15) are implementation-defined and documented.
Results that depend on the end conditions are portable only between implementations that document the same choice. `ClockGen` (Appendix D.2) depends on the generator, which is now fixed. This is an exception to §0.1.1, beside G.46.
For review: fix one pair of end conditions.

### G.65 The `"cosim"` interface

**Status: Closed.**
The C interface of an extern module, `"cosim"` included, is the device ABI, whose exported symbol is `piperine_abi_1` (§8.5.8). `"cosim"` ports stay discrete-only (§8.5.11); continuous ports on a `"cosim"` module are not accepted.
In force: the contract of §8.5, and §8.5.16 (a step receives and commits the bytes of the types its ports declare). `"osdi"` is OSDI 0.4 (§8.5.8), reached through an adapter over the same ABI.

### G.66 Session parameter names of `simparam`

**Status: Open.**
`simparam(name, default)` (§14.4) reads a session parameter. This specification names none, so no name is portable between hosts.

### G.67 Two blanket traits with one method name

**Status: Open.**
A method call on a carrier links a trait method through a blanket `impl<T: Repr> Tr for T` of a trait in scope at the call (§22, STD-06). No section gives a code for two traits in scope that both have the method name.
In force: the first in declaration order links, with no diagnostic. For review: report an ambiguity.

### G.68 `Tran` passed to `analysis` names a module

**Status: Open.**
`Transient` is the analysis kind (§22.3.7); `Tran` is the module that places a transmission gate in `std::nets`. Name resolution links `Tran` written as the argument of `analysis` to that module without a diagnostic, because the POM has no type checking: an argument of `analysis` is not required to be an `Analysis` value.
In force: the specification spells the kind `Transient` everywhere. A later type pass (msl-typeck) should reject a module as the argument of `analysis`.

### G.69 Implementation-defined float results

**Status: Resolved provisionally.** In force: the sign and payload of a NaN that an operation yields are implementation-defined (§1.1.8), and the elementary functions of H.2 rule 6 (`exp`, `ln`, the trigonometric and hyperbolic functions, `powf`, `powi`, `atan2`, `hypot`) are accurate to one unit in the last place and implementation-defined within it.
A NaN's bits are observable only through `encode` and `cast` (§2). A result that depends on the last place of an elementary function is portable only between implementations that compute it the same way. This is an exception to §0.1.1, beside G.46 and G.64.
For review: fix a canonical NaN, and require correctly rounded elementary functions or name one reference implementation.

### G.70 The rules of the value language formerly taken from Rust

**Status: Resolved provisionally.** Earlier revisions of this specification stated only what differs from Rust and left every other rule of the value language to Rust's own definition. This revision states them all (§0.4): Appendix A.7 to A.10, §1.1 to §1.8, §3.4.11 to §3.4.16, §3.7 to §3.9, Appendix B.3.7 and the rules under each table of Appendix H.
In force: the rules as written. Most restate what the earlier text implied. The ones that had to be decided, because the design never chose, are listed in §0.10 with the rule written for each.
For review: each row of §0.10.

### G.71 Value semantics

**Status: Implementation.** Decided (2026-11-20; V1 to V7 under "Closed items"): the value language has values only. Data are copied wherever they are used; hardware is reached by identity and changes only by a definition, a commit or a contribution.
In force: §1 rule 2, §1.2.5 to §1.2.7, §1.2.9, §1.4.6, §1.4.7, §1.5.9, §1.5.12, §1.5.14, §1.6.2, §1.6.7, §1.6.9, §1.7.5, §1.7.7, §3.1.10, §4 rule 8, §8.2.12, A.1, B.3.6, B.3.7 and H.4 to H.8.

This revision removes these special cases of earlier ones:

| Removed | Was stated in | What stands in its place |
| --- | --- | --- |
| the borrow type `&T`, allowed in a parameter type for an array, a slice or a string, and on any call argument | §1 rule 2, §1.2.6, `E0102` | the value: a parameter of type `T` (§1.2.6) |
| the four places where a borrow was read through without an operator, and `*r` elsewhere | §1.2.6, §1.2.7 | nothing to read through |
| the slice type `[T]`, only behind a borrow; `&[T; N]` accepted where `&[T]` was required | §1.2.5 | arrays; a const parameter `N` where the length varies (§1.2.5) |
| `e[a..b]` usable only as a borrowed argument or a receiver | §1.5.9 | `e[a..b]` is an array, with const bounds (§1.5.9) |
| the lifetime `'static`, and `&'static str` as the one borrow a `const` could hold | §1.2.7, §1.2.9, A.1 | `str`, a value type (§1.2.9); no lifetime token |
| the `Copy` property, held by some types and not by others; the move of a value without it; no field read out of such a value | §1.4.6, §1.4.7, `E0183` | every value is copied (§1.4.6) |
| the traits `Clone` and `Copy`, their derives, `clone()`; the `Copy` requirement of `[x; N]`, of `*r` and of `Repr` | H.8, §1.2.4, §2 | none: the names do not resolve (`E0161`, H.8 rule 1) |
| `let mut`, `mut x` in a pattern, and a local assigned at most once without it | §1.5.12, §1.6.2, §1.7.5, §1.7.7 | every local is a place (§1.5.12) |
| `let mut` reported separately in a module body | §8.2.12, `E0331` | a body `let` is not a place (`E0183`); `mut` is a reserved word (`E0903`) |
| `ref x` and `&p` patterns; a pattern that meets a borrow binds borrows | §1.6.2, §1.6.9 | a pattern binds copies (§1.6.9) |
| `move` before a closure, accepted with no effect | §1.5.14 | a closure captures by value; `move` is a reserved word |
| the `&self` spelling of Appendix H, for a method that did not move its receiver | H rule 4 | `self` in every signature |
| `resolve` typed by a function pointer over a slice, `fn(&[T]) -> T` | §4 rule 8 | a function with a const parameter, `fn f<const N: usize>(drives: [T; N]) -> T` |

Two codes are retired: `E0102` (§1) and `E0331` (§8.2). `E0183` no longer has a move to report.

The reference implementation does not yet meet this item. Its front end still accepts `&`, `*r`, `mut`, `ref`, `move`, `&'static str` and the derives `Clone` and `Copy`, and still reports `E0331`. The source files of `std`, and so the fences of Part VII that are excerpts of them (§22), are still written in the earlier spelling. The feature `value-semantics` brings the implementation, `std` and those fences to the rules above.
For review: that a parameter is not a place (§1.5.12, §3.1.10), so a method cannot assign `self`; and that `resolve` takes its drives as an array with a const length (§4 rule 8).

### G.72 Explicit unknowns

**Status: Implementation.** Decided (2026-11-21, user: "Não acho que unknown deveria ser algo implícito, deveria ser algo EXPLICITO"; rules R1 to R10 of the feature `explicit-unknowns`): no value carries a hidden known flag. What a type may hold is what its declaration says.
In force: §1 rule 13, §2, §3.1.9, §3.3, §3.7.13, §4 rules 1, 2, 6, 8 and 15, §5.1 to §5.3, §8.2.3, §13 rules 1 and 6, §19, §23.1 rule 9, §22.1, §22.3, Appendix H.8 and H.10.

This revision removes these special cases of earlier ones:

| Removed | Was stated in | What stands in its place |
| --- | --- | --- |
| the known flag of every level of every value, and the literal `Unknown` | §3.3, A.3, B.1.2 | `bool`, the integers, the floats, enums, structs, tuples and arrays are always determined (§3.3.1); `Logic<N>` says "may be X or Z" (§22.1.1); `Option<T>` says "may be absent" (§3.3.3) |
| `level`, `known`, `known_or`, `is_known`, `high`, `merge`, and `Known` | §3.3.4, §22.1.1, §22.1.2 | names that do not resolve (`E0161`); `to_bits()`, `is_high()`, `same`, `match` on a `Logic` or an `Option` |
| the merge of effects under unknown control, its loop bound, an unknown index, an unknown delay | §3.3.3, §3.3.13, §3.3.16, §12.1.19, Appendix C.2 | control takes a `bool` (§3.3.5): one path runs |
| the gate of an `analog` read and the flag-total functions that passed it | §3.3.5 to §3.3.7, `E0150` | `analog` reads a discrete carrier directly, a `Logic` through a function (§3.3.6) |
| an unknown structural decision, `E0151` | §3.3.8 | a `bool` or an integer (§3.3.7) |
| a carrier with no value until its first commit | §5.1.1, §5.3.8 | the initial value, written or `Default::default()`, is committed at `(0, 0)` (§3.3.4); a type without `Default` requires it (`E0187`); no primitive numeric type implements `Default` |
| the first commit of a carrier that starts high being a rise | §13 rule 6, §22.1.5 | a rise is a change that ends at true; a carrier that starts high has not risen (`init()` marks the start) |
| `maybe` on a record, an unknown `pass`, a `finish` that goes on under unknown control | §19, §22.6, §23.2 | a record is its declared fields; a `require` is a pass or a fail; a `finish` ends the analysis |
| the `resolve` field of a nature, and the library nets `Wire`, `Tri` and `SumNet` | §4, §22.3 | the trait `Resolve<X, V>` that a nature implements (§4 rule 8); a net nature is declared by the model (Appendix F.4) |
| "a loop of resolved drives that does not settle is not an error" | §8.2.3 | `E0511`: a conflict is a value, an instability is an error |

Four codes are retired: `E0150`, `E0151` (§3.3), `E0906` and `E0908` (A.3). `E0134` is narrowed to a bundle, `E0187` gains the three `Default` conditions, `E0220` names the nature and `Resolve`, and `E0225` and `E0511` are new. Items G.28, G.29, G.45, G.46, G.55 and G.57 closed with the model they concerned.

The reference implementation does not yet meet this item. It still carries one known byte per value in its IR, its plan, its layouts, its generated code and its device contract, runs a merge under unknown control, reads `maybe` in its records, and has no `Resolve` trait or `Logic` library type; `GAPS.md` S8 tracks it. The source files of `std`, and so the fences of Part VII that `std_excerpts` lists as ahead of them, are still written in the earlier model.
For review: that no primitive numeric type implements `Default`, which differs from Rust on purpose (§3.3.4); and that a rise is a change that ends at true, so a carrier that starts high has not risen (§22.1.5).

## Closed items

### Closed in this specification

| Item | Question | Decision | Where |
| --- | --- | --- | --- |
| G.40 | What an analysis run at a transient's last instant keeps | everything: time, the event queue, discrete state, instance state, the analog history. A `Transient` there is the same transient continued; an analysis without time is one more instant. A periodic steady state is continued transients only. | §16.4, §16.3 |
| G.41 | Whether a sampled real value has a sensitivity | a commit of a real value carries the sensitivity of the expression committed, at the commit's instant. The same rule gives the periodic small-signal response. | §23.3.4, §11.1 |
| G.42 | How the solve tables read in a periodic analysis | the small-signal column is taken at each point of the trajectory, with `now()` its time. `zi_nd` is in its `delay` form there, on purpose. | §16.2, §16.3 |
| G.43 | A hold that agrees with a potential contribution on the same node | always an error: a hold is a potential source, and a loop of potential contributions is singular whether or not the values agree | §16 |
| G.44 | Host actions between two runs of a continued transient; the old stop time | a host `set` or drive is a commit collected at the resume instant. The old stop time remains a time point. | §16.4.1 |
| D1 | Which forms a reader of Rust would expect are not in the language | Appendix B.3.6 is the complete list of what is removed: type aliases, associated types, `static` items, trait-bound `where`, `impl Trait`, qualified paths, restricted `pub(..)`, loop labels, pattern parameters, references and the markers `mut`, `ref` and `move` (V1, V2). In: function-pointer types `fn(T) -> U` and a negative literal as a const generic argument. | B.3.6, §1 rule 2, A.1 |
| D2 | Which library names exist without a declaration | a closed catalogue: numeric methods of floats and integers, `Option` combinators, arrays, ranges, ten iterator adaptors, derivable traits. Everything else is `E0161`. | Appendix H, §3.4.2 |
| D3 | `gnd`, `Nature`, `m`, `dtemp` and `str` | core names, in the built-in table, that no declaration can define. | §6 rule 11, §3.4.2, A.1 rule 8 |
| D5 | Duplicate names, colliding imports, ambiguous globs | `E0162`, with these rules: one name per table per namespace; an item or an explicit `use` beats a glob; two globs are an error only when the name is used. | §3.4.7, §3.4.8, §3.4.9 |
| D6 | Generic names of a behavior block | the block repeats the module's generic names in the same order and introduces none of its own. | §10 rule 6, B.3.3 item 14 |
| D7 | `&` in argument position | superseded by V1: no position takes a borrow, an argument is the value, and `E0102` is retired. | §1 rule 2, §3.2 |
| D8 | What `E0910` counts | nesting only (delimiters, blocks, unary and type nesting) up to 256 levels; flat chains of binary operators, methods and `else if` arms have a separate bound of 4096 links. | A.6 |
| D9 | Where docs and attributes may be written | before every item, field, variant, `match` arm and statement, and before reactions, definitions, generation and nature quantities; the docs of a quantity and of a reaction are metadata, those of a statement are kept and ignored. | A.4, §21 |
| D10 | Identifier patterns | a plain identifier pattern that resolves to a constant, a unit variant or a unit struct is that path, not a binding. | §1 rule 14 |
| D11 | Structural checks the elaborator reports | `E0320`, `E0140`, `E0370`, `E0201` and `E0263` are reported under the specification's codes, at name resolution. | §8.5.7, §3.2.9, §8.4, §4, B.3.3 |
| D12 | Out-of-range literals | `E0909`, at parse: a float literal whose nearest `f64` is infinite; an integer literal past its suffix type, or past 128 bits without a suffix. | A.2 rule 8, A.6, §1 rule 7 |
| D13 | Where items may sit | at namespace scope, in a file or an inline `mod`; a hardware module and its blocks are in one file, any namespace of it. | §3.4.3, B.2 |
| D14 | Codes of misplaced statements | `E0404` for `<-`, `+>`, `on`, `bound_step` and `signal` in any block or body that is not theirs, a module body included; `E0121` for a `fn` body, where it wins. | §10 rule 8, §3 rule 6, A.6 |
| D15 | Raw identifiers (G.15) | `r#name` is accepted for the seven added keywords and for no other word. | A.1 rule 6, G.15 |
| D16 | Field-init shorthand (G.14) | `{ r }` is accepted in param blocks and templates as in a struct expression (§1.3.3), and the tree keeps the authored form. | B.3.2, G.14 |
| D17 | `(s)` as a place | a place is a path with indexes and fields; parentheses are rejected in a definition and in a commit (`E0900`). | B.3.2 note 4 |
| P1 | Where a crate starts and what its files are called | the crate root is `src/lib.msl` and a source file has the extension `.msl`; `mod name;` loads `name.msl` or `name/mod.msl`. | §3.6 rule 2 |
| P2 | A dependency name against a local name | a dependency name is the first segment of a path below the names of the enclosing namespace; a local type-namespace name wins in an ordinary path and clashes in a `use` (`E0163`); a value item of that name hides nothing; `extern crate` is removed. | §3.5 rules 4, 5, 11 |
| P3 | Where `std` comes from | `std` is a dependency of every crate that no manifest names; the prelude is `std::prelude`, implied in every file. | §3.5 rules 3 and 9, §3.4.2 |
| P4 | Which manifest keys exist | the manifest holds the tables and keys of one closed table; any other is `E0171`. | §3.6 rule 3 |
| P5 | How a git dependency is pinned | one of `branch`, `tag` or `rev`, or the default branch; the lockfile records the commit and is honoured while the source is unchanged. | §3.6 rules 7, 13, 14 |
| P6 | When the manifests are read | in a fifth phase, `package`, before parse and elaboration; a model that meets its conditions is rejected. | §0.3 |
| S1 | How `std` declares an item the engine implements | `extern "intrinsic" fn`, a second ABI string accepted only in the crate `std`: no body, any types, no `#[derivative]`. | §3 rule 10, §22 rule 4 |
| S2 | Which items of `std` are written in MSL | every item the language can express has a body; the five that read the characters of a `str` (`matches_bits`, `logic`, `table_model`, `parse_table`, `parse_hex`) stay `extern "intrinsic"`. | §22 |
| S3 | The analysis kind and the pass switch share a name | the analysis kind is `Transient`; the module `Tran` of §22.3.7 keeps its name. | §11.1.1, §22.3.7 |
| S4 | The namespaces of `std` | `std::constants`, `std::carriers`, `std::values`, `std::analog`, `std::nets`, `std::convert`, `std::monitor`, `std::io`, all re-exported by `std::prelude`. | §22 |
| S5 | Where the prelude constants live | `Q`, `K_B`, `EPS0`, `PI`, `E` and `inf` are `pub const` items of `std::constants`; `inf` is `1.0 / 0.0`. | §1 rule 3 |
| V1 | Whether data are passed by value or by reference | by value, always: there is no reference, borrow or pointer, and no lifetime. `&` is the bitwise operator only. | §1 rule 2, §1.2.6, §1.4.6 |
| V2 | Which locals may be assigned | every one: `mut` is gone, with `ref` and `move`; the three are reserved words (`E0903`). A parameter, `self` included, is not a place. | §1.5.12, §1.7.7, A.1 |
| V3 | What happens to a value used twice | it is copied: no move, no `Copy`, no `Clone`, no `clone()`. The two names are not accepted as empty derives: they do not resolve (`E0161`). | §1.4.6, H.8 rule 1 |
| V4 | Slices | none. An array is a value; a function over any length takes `const N: usize`; `e[a..b]` is the array `[T; b - a]`, with const bounds; `resolve` takes `[T; N]`. | §1.2.5, §1.5.9, §4 rule 8 |
| V5 | Strings | `str` is a value type, the text of a literal; `&str`, `&'static str` and the lifetime `'static` are gone. | §1.2.9, A.1 |
| V6 | Methods that change a value | `self` is a copy; the method returns the new value and the caller assigns it, `s = s.step(x)`. There is no shorthand. | §3.1.10 |
| V7 | What is reached by identity | hardware only: carriers, nodes, instances and bundles, through ports, connections and handles. Shared state changes only by a definition, a commit or a contribution, at the time its rule gives. | §1.4.7, §3.1 |

G.33 to G.36 are what remains open around G.40 and G.42. D1 to D17 number the decisions taken when this specification's front end was audited against the grammar; D4, whether the standard library is a normative source, is not among them (G.38). P1 to P6 number the decisions of the crate and package model ([§3.5](part_i_values.md#35-crates), [§3.6](part_i_values.md#36-packages)). S1 to S5 number the decisions of the standard library ([§22](part_vii_stdlib.md#22-standard-library)). V1 to V7 number the decisions of value semantics (G.71).

### Other closed items

| Item | Closed by |
| --- | --- |
| G.2, G.3, G.25 | no residue in this specification |
| G.7 | `inertial` (§22.5.8) |
| G.12 | `A2D` (§22.4.2) |
| G.18 | bundles and `Fn(P)` (§7, §8.4) |
| G.26 | `above` (§22.2.5) |
| G.27, G.31 | relaxation of loops among resolved drives (§8.2.3) |
| G.28, G.29, G.45, G.46, G.55, G.57 | the explicit model of unknowns: nothing is merged and nothing is gated (G.72) |

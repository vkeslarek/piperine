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
| the SystemVerilog verification language | the testbench lives in the host (§23.1) |
| blocking writes and `#0` | commits; every read sees the past (§12.1.1) |
| indirect contributions | the row of an internal node (§11, `Nullor`) |
| heap types | arrays and carriers fixed at elaboration (§1 rule 2) |
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
| G.28 | Cost of unknown control | Provisional |
| G.29 | `known_or` on a module's own state | Provisional |
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
| known flags | §3.3 |
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
A stdlib synchronizer whose first stage commits `Unknown` when `since(d) < tw` at the edge. It needs no language change (§22.5.1 rule 4 shows the pattern).

### G.14 Field-init shorthand

**Status: Closed.**
`{ r }` for `{ r: r }` in instances and templates is accepted, as in a Rust struct expression, and the tree keeps the authored form (Appendix B.3).

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

### G.28 Cost of unknown control

**Status: Provisional.** In force: an unknown address merges into every word it could select (§3.3.3, Appendix E.5). This is exact and can be expensive on a large memory.
The elaborator may warn; the semantics stays.

### G.29 `known_or` on a module's own state

**Status: Provisional.** In force: §3.3.5 has no exception. It asks for `known_or` even on a signal the module always commits known (`seg` in Appendix D.1).
A static proof of known-ness could waive it. The rule without exceptions was preferred.

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
The language adds 65 constructs to Rust (Appendix B.1) for 11 concepts (§0). No further construct has been found that can be removed without a model paying for it.

## Flagged resolutions

These were silent in the design. A part resolved each with a `Resolution` note, which is normative. They are listed for review because each affects portability between implementations.

| Item | Resolved in | In force | For review |
| --- | --- | --- | --- |
| G.45 Panic under unknown control | §1 rules 11–13 | a value-level failure is a panic; on a path taken under unknown control it is not an error, and every effect of that merge becomes unknown | whether a possible panic should be reported, at least on `log` |
| G.46 Bound on loop unrolling under unknown control | §3.3.16 | the bound of §3.3.3 is implementation-defined and documented; implementations may differ only in how many levels stay known | whether to fix the bound |
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
| G.55 | §19 rule 5, §22.6.2 | a `finish` record ends the analysis only if known true and not `maybe` | a `fatal()` under unknown control does not stop the run |
| G.56 | §8.3.6 | every core result has a direct dependence on its value inputs | `let y = delay(f(y), td);` is an error |
| G.57 | §3.3.6, §3.3.7 | the `analog` check is syntactic over flag-total functions; four stdlib functions are mandatory | a new term that carries a conformance rule |
| G.58 | §11 rule 3 | the selector rule also binds the flow contributions of a potential-capable branch; a probed short is not merged | extends the rule, with one exception clause |
| G.59 | §6 rule 5 | `p.inflow()` includes the branches of child instances that end on `p` | the design says "this module's branches" |
| G.60 | §15 T2, §15 T3 | a half-tick tie rounds to the later tick; a positive delay that rounds to no advance is the next microstep | observable at half-tick delays |
| G.61 | A.1, A.3, §1 | `Unknown` is a reserved literal; an unlisted attribute is an error; `char` and every lifetime but `'static` are dropped | narrows "close to Rust" |
| G.62 | §1 | `usize` and `isize` are 64 bits on every platform | differs from Rust; needed for §0.1.1 |
| G.63 | §23.1 | a `force` on a node is a potential source to `gnd`; a host action answering an event is due at `(t, n + 1)` | new host semantics |

### G.64 Implementation-defined numerics

**Status: Resolved provisionally.** In force: the generator of `normal` is the one written in `std::analog` (§22.2.17): SplitMix64 for two uniform draws and Box-Muller for the sample. The end conditions of the splines of `table_model` (§22.2.15) are implementation-defined and documented.
Results that depend on the end conditions are portable only between implementations that document the same choice. `ClockGen` (Appendix D.2) depends on the generator, which is now fixed. This is an exception to §0.1.1, beside G.46.
For review: fix one pair of end conditions.

### G.65 The `"cosim"` interface

**Status: Closed.**
The C interface of an extern module, `"cosim"` included, is the device ABI, whose exported symbol is `piperine_abi_1` (§8.5.8). `"cosim"` ports stay discrete-only (§8.5.11); continuous ports on a `"cosim"` module are not accepted.
In force: the contract of §8.5, and §8.5.16 (a step with an unknown input is not run; every `out` port commits `Unknown`). `"osdi"` is OSDI 0.4 (§8.5.8), reached through an adapter over the same ABI.

### G.66 Session parameter names of `simparam`

**Status: Open.**
`simparam(name, default)` (§14.4) reads a session parameter. This specification names none, so no name is portable between hosts.

### G.67 Two blanket traits with one method name

**Status: Open.**
A method call on a carrier links a trait method through a blanket `impl<T: Repr> Tr for T` of a trait in scope at the call (§22, STD-06). No section gives a code for two traits in scope that both have the method name.
In force: the first in declaration order links, with no diagnostic. For review: report an ambiguity, as Rust does.

### G.68 `Tran` passed to `analysis` names a module

**Status: Open.**
`Transient` is the analysis kind (§22.3.7); `Tran` is the module that places a transmission gate in `std::nets`. Name resolution links `Tran` written as the argument of `analysis` to that module without a diagnostic, because the POM has no type checking: an argument of `analysis` is not required to be an `Analysis` value.
In force: the specification spells the kind `Transient` everywhere. A later type pass (msl-typeck) should reject a module as the argument of `analysis`.

## Closed items

### Closed in this specification

| Item | Question | Decision | Where |
| --- | --- | --- | --- |
| G.40 | What an analysis run at a transient's last instant keeps | everything: time, the event queue, discrete state, instance state, the analog history. A `Transient` there is the same transient continued; an analysis without time is one more instant. A periodic steady state is continued transients only. | §16.4, §16.3 |
| G.41 | Whether a sampled real value has a sensitivity | a commit of a real value carries the sensitivity of the expression committed, at the commit's instant. The same rule gives the periodic small-signal response. | §23.3.4, §11.1 |
| G.42 | How the solve tables read in a periodic analysis | the small-signal column is taken at each point of the trajectory, with `now()` its time. `zi_nd` is in its `delay` form there, on purpose. | §16.2, §16.3 |
| G.43 | A hold that agrees with a potential contribution on the same node | always an error: a hold is a potential source, and a loop of potential contributions is singular whether or not the values agree | §16 |
| G.44 | Host actions between two runs of a continued transient; the old stop time | a host `set` or drive is a commit collected at the resume instant. The old stop time remains a time point. | §16.4.1 |
| D1 | Which Rust forms are in the language | Appendix B.3.6 is the complete list of what is removed: type aliases, associated types, `static` items, trait-bound `where`, `impl Trait`, qualified paths, restricted `pub(..)`, loop labels, pattern and `mut` parameters, `&self`. In: function-pointer types `fn(T) -> U` and a negative literal as a const generic argument. | B.3.6, §1 rule 2, A.1 |
| D2 | Which Rust library names exist | a closed catalogue: numeric methods of floats and integers, `Option` combinators, arrays and slices, ranges, ten iterator adaptors, derivable traits. Everything else is `E0161`. | Appendix H, §3.4.2 |
| D3 | `gnd`, `Nature`, `m`, `dtemp` and `str` | core names, in the built-in table, that no declaration can define. | §6 rule 11, §3.4.2, A.1 rule 8 |
| D5 | Duplicate names, colliding imports, ambiguous globs | `E0162`, with Rust's rules: one name per table per namespace; an item or an explicit `use` beats a glob; two globs are an error only when the name is used. | §3.4.7, §3.4.8, §3.4.9 |
| D6 | Generic names of a behavior block | the block repeats the module's generic names in the same order and introduces none of its own. | §10 rule 6, B.3.3 item 14 |
| D7 | `&` in argument position | a shared borrow of any value is allowed as a call argument; `E0102` covers the rest. | §1 (`E0102`), §3.2 |
| D8 | What `E0910` counts | nesting only (delimiters, blocks, unary and type nesting) up to 256 levels; flat chains of binary operators, methods and `else if` arms have a separate bound of 4096 links. | A.6 |
| D9 | Where docs and attributes may be written | everywhere Rust takes an outer attribute, and before reactions, definitions, generation and nature quantities; the docs of a quantity and of a reaction are metadata, those of a statement are kept and ignored. | A.4, §21 |
| D10 | Identifier patterns | a plain identifier pattern that resolves to a constant, a unit variant or a unit struct is that path, not a binding, as in Rust. | §1 rule 14 |
| D11 | Structural checks the elaborator reports | `E0320`, `E0140`, `E0370`, `E0201` and `E0263` are reported under the specification's codes, at name resolution. | §8.5.7, §3.2.9, §8.4, §4, B.3.3 |
| D12 | Out-of-range literals | `E0909`, at parse: a float literal whose nearest `f64` is infinite; an integer literal past its suffix type, or past 128 bits without a suffix. | A.2 rule 8, A.6, §1 rule 7 |
| D13 | Where items may sit | at namespace scope, in a file or an inline `mod`; a hardware module and its blocks are in one file, any namespace of it. | §3.4.3, B.2 |
| D14 | Codes of misplaced statements | `E0404` for `<-`, `+>`, `on`, `bound_step` and `signal` in any block or body that is not theirs, a module body included; `E0121` for a `fn` body, where it wins. | §10 rule 8, §3 rule 6, A.6 |
| D15 | Raw identifiers (G.15) | `r#name` is accepted for the seven added keywords and for no other word. | A.1 rule 6, G.15 |
| D16 | Field-init shorthand (G.14) | `{ r }` is accepted in param blocks and templates as in a Rust struct expression, and the tree keeps the authored form. | B.3.2, G.14 |
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

G.33 to G.36 are what remains open around G.40 and G.42. D1 to D17 number the decisions taken when this specification's front end was audited against the grammar; D4, whether the standard library is a normative source, is not among them (G.38). P1 to P6 number the decisions of the crate and package model ([§3.5](part_i_values.md#35-crates), [§3.6](part_i_values.md#36-packages)). S1 to S5 number the decisions of the standard library ([§22](part_vii_stdlib.md#22-standard-library)).

### Other closed items

| Item | Closed by |
| --- | --- |
| G.2, G.3, G.25 | no residue in this specification |
| G.7 | `inertial` (§22.5.8) |
| G.12 | `A2D` (§22.4.2) |
| G.18 | bundles and `Fn(P)` (§7, §8.4) |
| G.26 | `above` (§22.2.5) |
| G.27, G.31 | relaxation of loops among resolved drives (§8.2.3) |

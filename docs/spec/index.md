# Piperine mixed-signal language: specification

This document specifies the Piperine mixed-signal language (MSL). It is the entry point of the
specification: scope, notation, conformance terms, the concept table, the file map and the
glossary index.

## 0. Concepts

### 0.1 Scope

MSL is a language for mixed-signal models. It is similar to Rust and based on it, but it is not
Rust. It describes digital, analog and mixed circuits for simulation and optimization.

1. Every outcome of a simulation is a function of the source and of the host's actions alone.
   There are no races ([§12.1](part_iv_behavior.md#121-reactions)).
2. A model reuses existing models through `extern` modules: Verilog-A compiled to OSDI, and
   other simulators by cosimulation ([§8.5](part_iii_structure.md#85-implicit-params-and-extern-modules)).
3. Testbenches, stimulus sequences, sweeps and optimization live in the host, a program
   outside the language ([§23](part_viii_host.md#23-testbenches-and-optimization)).
4. What is out of scope, and every open question, is listed in
   [Appendix G](appendix_g_scope.md). An open item has an id `G.n`.

Two things are deliberately not concepts. Output is one statement, `emit!`
([§19](part_vi_io.md#19-streams-and-checks)): no model reads it back. The host is outside the
language: it sets params, drives the top ports and reads everything.

### 0.2 Notation

**Section numbers.** Sections are numbered §1 … §23 across Parts I–VIII. Numbers are stable
across files. Appendices are lettered A–G; their items are cited as `A.2`, `B.3`, `G.6`.

**Rule numbers.** One convention holds in every file.

| Form | Cites |
| --- | --- |
| `§8.2.3` | rule 3 of subsection 8.2 |
| `§11 rule 3` | rule 3 of a top-level section; `§11.3` would name a subsection |
| `§15 step 3.4`, `§16 step 2` | a step of the procedure of §15 or §16 |
| `§15 T2`, `§15 I3`, `§15 S1`, `§16 O2`, `§16 H4`, `§17.2 A3` | an item of a lettered list of [Part V](part_v_execution.md): ticks, invariants, settle commit, operating point, held nodes, advance |
| `§22.2.12 rule 1` | a rule of a standard-library entry |
| `A.2 rule 3`, `B.3.3 item 4` | a rule or item of an appendix |

Rule numbers are identifiers. Within a section the rules are grouped under *Static semantics*
and *Dynamic semantics*, so they need not appear in ascending order. Each rule appears once.

**Code font.** `monospace` marks source text: keywords, operators, names, literals and
grammar symbols. An example block is MSL source unless it is labelled otherwise.

**Defined terms.** A term is set in **bold** at its single defining occurrence. Every other
use refers to that definition. [§0.9](#09-glossary-index) lists where each term is defined.

**Grammar.** Syntax is given in EBNF. The complete grammar of the additions to Rust is
[Appendix B.3](appendix_b_grammar.md); excerpts in the parts repeat it verbatim.

| Notation | Meaning |
| --- | --- |
| `name ::= …` | a production |
| `'text'` | a terminal: the token spelled `text` |
| `IDENT`, `STRING` | a token class ([Appendix A](appendix_a_lexical.md)) |
| `a b` | `a` followed by `b` |
| `a \| b` | `a` or `b` |
| `( … )` | grouping |
| `x?` | zero or one `x` |
| `x*` | zero or more `x` |
| `gen<X>` | a production parameterized by the production `X` |
| `// …` | a comment, not part of the grammar |

The nonterminals `stmt`, `expr`, `type`, `pat`, `block`, `generics`, `generic_args`, `path` and
`attrs` belong to the value grammar. They are Rust's, with the changes of
[Part I](part_i_values.md).

**Notes.** A block that starts with `> **Resolution.**` marks a place where this specification
fixes a behavior that the design left vague or silent. Such a note is normative.

**Error codes.** An error condition has a code `Eppnn`: `pp` is the part that owns the rule,
`nn` a number inside it. Each error table gives the code, the condition and the phase.
A code names one condition, and a condition has one code, owned by the part that states the
rule. Another part's table lists that condition as a row keyed by the owning section, with
the code in parentheses. Numbers are not reused: a gap in a sequence is a retired code.

| Prefix | Owner |
| --- | --- |
| `E01xx` | [Part I](part_i_values.md) |
| `E02xx` | [Part II](part_ii_carriers.md) |
| `E03xx` | [Part III](part_iii_structure.md) |
| `E04xx` | [Part IV](part_iv_behavior.md) |
| `E05xx` | [Part V](part_v_execution.md) |
| `E06xx` | [Part VI](part_vi_io.md) |
| `E07xx` | [Part VII](part_vii_stdlib.md) |
| `E08xx` | [Part VIII](part_viii_host.md) |
| `E09xx` | [Appendix A](appendix_a_lexical.md) |

### 0.3 Conformance

The words **MUST**, **MUST NOT**, **SHALL**, **SHOULD** and **MAY** are used as in RFC 2119.

1. A **conforming implementation** accepts every conforming model and gives it the behavior
   this specification defines. An implementation that violates a **MUST** is non-conforming.
2. A **conforming model** is source text that raises no parse error and no elaboration error
   under this specification.
3. An implementation **MUST** reject a model that meets an error condition of phase *package*,
   *parse* or *elaboration*. It **MUST** fail the analysis when a condition of phase *run* is met. A
   condition of phase *host* refuses one host operation, or answers it with a diagnostic; the
   analysis continues.
4. A diagnostic **MUST** identify the source location or the hierarchical path the condition
   names. It **SHOULD** carry the error code.
5. A statement without a normative word that describes the language is normative. Examples,
   and text labelled as a note or as guidance, are informative.
6. Numeric defaults of the reference implementation ([Appendix C.2](appendix_c_implementation.md))
   are informative. The guarantees of [Part V](part_v_execution.md) are normative.

An error is raised in one of five phases.

| Phase | When | Defined in |
| --- | --- | --- |
| **package** | the manifests are read, the dependencies located and the crate graph fixed | [§3.6](part_i_values.md#36-packages) |
| **parse** | the source text is turned into tokens and a syntax tree | [Appendix A](appendix_a_lexical.md), [Appendix B](appendix_b_grammar.md) |
| **elaboration** | names and types are resolved, generics and generation are expanded, params are checked, and the netlist is fixed | [Part III](part_iii_structure.md), [§16](part_v_execution.md#16-the-operating-point) step 1 |
| **run** | an analysis executes | [Part V](part_v_execution.md) |
| **host** | the host asks for an operation: a `set`, a drive, a subscription, a gradient | [Part VIII](part_viii_host.md) |

> **Resolution.** Type checking, name resolution and every static check of this specification
> belong to the elaboration phase. A check that needs param values runs again on every host
> `set` it depends on ([§8.1](part_iii_structure.md#81-ports-and-params)).

### 0.4 Inheritance from Rust

**Everything this specification does not state is Rust, edition 2024, value semantics.** This
covers expressions, statements, patterns, types, traits, generics, name resolution, visibility,
integer and floating-point arithmetic, and the standard methods of the value types.

1. The specification states deltas only. A delta either adds a form
   ([Appendix B.1](appendix_b_grammar.md)) or removes one ([§1](part_i_values.md#1-value-types) rule 2).
   [Appendix B.3.6](appendix_b_grammar.md#b36-removed-from-rusts-grammar) lists every removed form.
2. Where a delta and Rust disagree, the delta holds.
3. The lexer and the grammar are the language's own ([Appendix A](appendix_a_lexical.md)). A
   conforming implementation need not accept any Rust program.
4. The library is the exception: the names a model takes from Rust's standard library are the
   closed catalogue of [Appendix H](appendix_h_library.md). A method, trait or type it does not
   list does not resolve (`E0161`).
5. "Rust's" in this specification means the behavior of *The Rust Reference* for edition 2024.

### 0.5 The eleven concepts

The language has eleven concepts. Each has one rule. The "Covers" column lists what that rule
decides. Nothing with a rule of its own is left out of the table.

| # | Concept | Rule | Covers | § |
| --- | --- | --- | --- | --- |
| 1 | value and `fn` | data with no time, every level known or unknown; a function keeps no state, writes no carrier and places no instance | `Repr`, the known flag, closures, `Event`-returning functions | [1](part_i_values.md#1-value-types), [2](part_i_values.md#2-repr), [3](part_i_values.md#3-functions) |
| 2 | nature | says what a carrier holds and how its drives combine | `Bit` (one drive), resolved natures (`resolve`), continuous natures (KCL) | [4](part_ii_carriers.md#4-natures) |
| 3 | carrier | a signal or port of a nature: it exists over time, and is connected by being passed to a port | discrete carriers, nodes, bundles (a `struct` of carriers) | [5](part_ii_carriers.md#5-discrete-carriers), [6](part_ii_carriers.md#6-continuous-carriers), [7](part_ii_carriers.md#7-bundles) |
| 4 | drive | what one module writes to one carrier; the carrier's value is its nature's combination of the drives | definitions (`=`), commits, initial values, `others()`; on a node, the module's branches | [5.2](part_ii_carriers.md#52-drives), [8.2](part_iii_structure.md#82-the-body) |
| 5 | param | a value fixed during a solve; the model reads it, the host sets it | defaults, `where`, structural params | [8.1](part_iii_structure.md#81-ports-and-params), [9](part_iii_structure.md#9-quantities) |
| 6 | module | ports, params and a netlist of signals and instances, fixed at elaboration; with its params bound it is a value, and calling that value places an instance | templates, generation, `extern` modules, `m`, `dtemp`, every operator with state (`ddt`, `cross`, `idt`, …) | [8](part_iii_structure.md#8-modules) |
| 7 | `let` | names an expression, evaluated wherever the name is read | value inputs and results (a `let` across the instance boundary), instances inside expressions | [8.2](part_iii_structure.md#82-the-body), [8.3](part_iii_structure.md#83-instances-inside-expressions) |
| 8 | `analog for M` | `probe +> x` adds `x` to the flow or to the potential of a branch, a pair of nodes, at every Newton iteration | switch branches, small-signal, noise | [11](part_iv_behavior.md#11-analog) |
| 9 | `digital for M` | a reaction `on ev { q <- x; }` runs once when `ev` fires; its commits apply at the next microstep | `after` | [12](part_iv_behavior.md#12-digital) |
| 10 | event | a value of type `Event`: it fires at instants | `changed`, `at`, `cross`, `settle`, `\|\|`, guards | [13](part_iv_behavior.md#13-events) |
| 11 | instant | `(t, n)`: at one time, microsteps run until nothing is pending; `analog` is solved before the first, and again after each one that changes what it reads | the operating point, the jump, time points, what a `settle` commit is | [15](part_v_execution.md#15-time-and-the-instant), [16](part_v_execution.md#16-the-operating-point), [17](part_v_execution.md#17-transient) |

**State is structure, and there are two kinds of callable.** A `fn` keeps no state. What keeps
state is a signal declared in a module. Anything with state is therefore a module: every
operator that keeps state (`ddt`, `delay`, `cross`, `idt`, `slew`, `transition`, …) is a module
with a result, core or stdlib ([§8.3](part_iii_structure.md#83-instances-inside-expressions)).

### 0.6 Who writes what

| Form | Target | Written in | Takes effect |
| --- | --- | --- | --- |
| `y = expr [after d];` | a discrete carrier: this module's drive of it, a definition | module body | in the microstep its inputs change, before reactions run; with `after`, as a commit ([§8.2](part_iii_structure.md#82-the-body)) |
| `branch +> expr;` | a branch: its potential or its flow | `analog` | in the current Newton iteration ([§11](part_iv_behavior.md#11-analog)) |
| `q <- expr [after d];` | a discrete carrier: a commit to this module's drive | a reaction | at the next microstep, or `d` later ([§12.1](part_iv_behavior.md#121-reactions)) |

1. Each of `=`, `+>` and `<-` has one meaning.
2. `let x = expr;` writes nothing: it names an expression.
3. Nets are not joined by a statement. One carrier is passed to both ports.
4. Params have no assignment form. Models read them; the host sets them.

### 0.7 How a model reads

```rust
/// A comparator with hysteresis that drives a flip-flop.
module Front(vin: Electrical, in clk: Bit, out q: Bit) {
    vth: f64 = 0.6;
    hyst: f64 = 50m;
    signal above_th: Bit = false;        // state: a signal
    signal d: Bit;
    d = above_th;                        // structure: a combinational definition
}

digital for Front {
    on above(v(vin) - vth - hyst / 2.0) { above_th <- true; }    // an analog event
    on above(vth - hyst / 2.0 - v(vin)) { above_th <- false; }
    on clk.rise() { q <- d; }                                    // a digital event
}
```

The module body is the netlist: ports, params, two signals, one definition. Behavior is in the
`digital` block: three reactions, two on crossings of the input and one on the clock. `above`
is a stdlib module with a result of type `Event` ([§22.2](part_vii_stdlib.md#222-analog-functions-and-modules));
each use is an instance. Nothing in the model keeps state outside its signals. This instant is
traced in [§15.1](part_v_execution.md#151-an-instant-traced).

**Reading guide.**

| Reader | Start with |
| --- | --- |
| model author, digital | §0, [Part II](part_ii_carriers.md) §5, [Part III](part_iii_structure.md), [Part IV](part_iv_behavior.md) §12–13, [Appendix E](appendix_e_gallery_digital.md) |
| model author, analog | §0, [Part II](part_ii_carriers.md) §4 and §6, [Part IV](part_iv_behavior.md) §11, [Part V](part_v_execution.md) §16–18, [Appendix D](appendix_d_gallery_analog.md) |
| mixed-signal author | both of the above, then [§15](part_v_execution.md#15-time-and-the-instant) and [Appendix F](appendix_f_gallery_mixed.md) |
| implementer of a front end | [Appendix A](appendix_a_lexical.md), [Appendix B](appendix_b_grammar.md), then Parts I–IV in order |
| implementer of a simulator | [Part V](part_v_execution.md), [Appendix C](appendix_c_implementation.md), then the per-solve table of [§16.2](part_v_execution.md#162-every-primitive-in-every-solve) |
| host author | [Part VI](part_vi_io.md), [Part VIII](part_viii_host.md) |

### 0.8 File map

| File | Content | Sections | Error codes |
| --- | --- | --- | --- |
| [`index.md`](index.md) | scope, notation, conformance terms, the concept table, who writes what, reading guide, file map, glossary index | §0 | — |
| [`part_i_values.md`](part_i_values.md) | value types, `Repr`, functions, `impl`, calls, unknown values, files | §1–3 | `E01xx` |
| [`part_ii_carriers.md`](part_ii_carriers.md) | natures, discrete carriers, drives, history, continuous carriers, bundles | §4–7 | `E02xx` |
| [`part_iii_structure.md`](part_iii_structure.md) | modules, ports, params, the body, instances in expressions, templates, `extern` modules, quantities | §8–9 | `E03xx` |
| [`part_iv_behavior.md`](part_iv_behavior.md) | behavior blocks, `analog`, `digital`, events, core primitives | §10–14 | `E04xx` |
| [`part_v_execution.md`](part_v_execution.md) | the instant, the jump, the operating point, analyses, transient, limiting | §15–18 | `E05xx` |
| [`part_vi_io.md`](part_vi_io.md) | streams and checks, input data, metadata | §19–21 | `E06xx` |
| [`part_vii_stdlib.md`](part_vii_stdlib.md) | the standard library | §22 | `E07xx` |
| [`part_viii_host.md`](part_viii_host.md) | testbenches, optimization, gradients across events | §23 | `E08xx` |
| [`appendix_a_lexical.md`](appendix_a_lexical.md) | lexical structure | App. A | `E09xx` |
| [`appendix_b_grammar.md`](appendix_b_grammar.md) | B.1 added to Rust, B.2 placement matrix, B.3 complete EBNF of the additions | App. B | — |
| [`appendix_c_implementation.md`](appendix_c_implementation.md) | lowering to AIR, the reference implementation | App. C | — |
| [`appendix_d_gallery_analog.md`](appendix_d_gallery_analog.md) | gallery: sources, clocks and continuous time | App. D | — |
| [`appendix_e_gallery_digital.md`](appendix_e_gallery_digital.md) | gallery: digital | App. E | — |
| [`appendix_f_gallery_mixed.md`](appendix_f_gallery_mixed.md) | gallery: mixed-signal systems | App. F | — |
| [`appendix_g_scope.md`](appendix_g_scope.md) | out of scope, and every open item `G.n` | App. G | — |
| [`appendix_h_library.md`](appendix_h_library.md) | the closed catalogue of the inherited library | App. H | — |

### 0.9 Glossary index

Each term has one defining occurrence. The table points at it.

| Term | Defined in |
| --- | --- |
| accepted step, rejected step | [§17.2](part_v_execution.md#172-steps) |
| active (of a `limit` instance) | [§18](part_v_execution.md#18-limiting) rule 5 |
| `analog` block | [§11](part_iv_behavior.md#11-analog) |
| analysis, `Analysis` | [§11.1](part_iv_behavior.md#111-one-block-every-analysis) |
| behavior block | [§10](part_iv_behavior.md#10-behavior-blocks) |
| branch | [§6](part_ii_carriers.md#6-continuous-carriers) |
| branch kind, selector | [§11](part_iv_behavior.md#11-analog) |
| bundle | [§7](part_ii_carriers.md#7-bundles) |
| carrier, signal | [Part II](part_ii_carriers.md) introduction |
| change (of a carrier) | [§5.3.4](part_ii_carriers.md#53-history) |
| check, reduction | [§19](part_vi_io.md#19-streams-and-checks) |
| closure | [§3](part_i_values.md#3-functions) rule 3 |
| commit | [§12.1](part_iv_behavior.md#121-reactions) |
| compatible natures | [§4](part_ii_carriers.md#4-natures) rule 13 |
| conforming implementation, conforming model | [§0.3](#03-conformance) |
| const context | [§8.1](part_iii_structure.md#81-ports-and-params) |
| continued transient | [§16.4.1](part_v_execution.md#164-from-one-analysis-to-the-next) |
| contribution | [§11](part_iv_behavior.md#11-analog) |
| converged | [§17.1](part_v_execution.md#171-convergence) |
| converter | [§22.4](part_vii_stdlib.md#224-converters) |
| core primitive | [§14](part_iv_behavior.md#14-core-primitives) |
| definition, delayed definition, anonymous definition | [§8.2](part_iii_structure.md#82-the-body) |
| `digital` block | [§12](part_iv_behavior.md#12-digital) |
| direct dependence | [§8.3.5](part_iii_structure.md#83-instances-inside-expressions) |
| discrete carrier | [§5](part_ii_carriers.md#5-discrete-carriers) |
| discrete expression, continuous expression | [§8.2](part_iii_structure.md#82-the-body) |
| discrete nature, continuous nature | [§4](part_ii_carriers.md#4-natures) |
| drive | [§5.2](part_ii_carriers.md#52-drives) |
| due, pending | [§15](part_v_execution.md#15-time-and-the-instant) |
| effective multiplicity | [§8.5.3](part_iii_structure.md#85-implicit-params-and-extern-modules) |
| equilibrium solve, transient step, jump solve, small-signal sweep | [§15](part_v_execution.md#15-time-and-the-instant), "Solves" |
| event position, guard | [§13](part_iv_behavior.md#13-events) |
| event time | [§15](part_v_execution.md#15-time-and-the-instant) |
| event, `Event` | [§13](part_iv_behavior.md#13-events) |
| extern module, implicit param | [§8.5](part_iii_structure.md#85-implicit-params-and-extern-modules) |
| flag-total function | [§3.3.6](part_i_values.md#33-unknown-values) |
| `force`, `release` | [§23.1](part_viii_host.md#231-testbenches) |
| function class | [§3](part_i_values.md#3-functions) |
| generation | [§8.2.7](part_iii_structure.md#82-the-body) |
| `gnd` | [§6](part_ii_carriers.md#6-continuous-carriers) rule 1 |
| gradient param | [§21](part_vi_io.md#21-metadata) rule 7 |
| gradient, sensitivity, saltation correction, grazing crossing | [§23.3](part_viii_host.md#233-gradients-across-events) |
| handle | [§3.1](part_i_values.md#31-impl) |
| held node, hold | [§16](part_v_execution.md#16-the-operating-point) H1 |
| high impedance, strength | [§22.3.1](part_vii_stdlib.md#2231-strength-logic) |
| host action, host drive, `set` | [§23.1](part_viii_host.md#231-testbenches) |
| host, session, testbench | [§23](part_viii_host.md#23-testbenches-and-optimization) |
| impulsive flow | [§15.2.2](part_v_execution.md#152-the-jump) |
| inertial delay | [§22.5.8](part_vii_stdlib.md#2258-inertial) |
| instance, placement | [§8.3](part_iii_structure.md#83-instances-inside-expressions) |
| instant, microstep | [§15](part_v_execution.md#15-time-and-the-instant) |
| jump, right value | [§15.2](part_v_execution.md#152-the-jump) |
| known, unknown, known flag | [§3.3](part_i_values.md#33-unknown-values) |
| left value | [§15](part_v_execution.md#15-time-and-the-instant) step 1 |
| level | [§2](part_i_values.md#2-repr) |
| limiting | [§18](part_v_execution.md#18-limiting) |
| located crossing | [§17.3](part_v_execution.md#173-locating-crossings) |
| loop, relaxation | [§8.2.3](part_iii_structure.md#82-the-body) |
| merge | [§3.3.3](part_i_values.md#33-unknown-values) |
| metadata, path | [§21](part_vi_io.md#21-metadata) |
| module | [§8](part_iii_structure.md#8-modules) |
| monitor | [§22.5](part_vii_stdlib.md#225-monitors) |
| named expression | [§8.2](part_iii_structure.md#82-the-body) |
| namespace module, prelude | [§3.4](part_i_values.md#34-files-and-imports) |
| nature quantity | [§4](part_ii_carriers.md#4-natures) rule 9 |
| nature, `Nature` | [§4](part_ii_carriers.md#4-natures) |
| node | [§6](part_ii_carriers.md#6-continuous-carriers) |
| nodeset | [§18](part_v_execution.md#18-limiting) |
| noise source | [§11.1.8](part_iv_behavior.md#111-one-block-every-analysis) |
| objective, constraint | [§23.2](part_viii_host.md#232-optimization) |
| one-driver carrier, resolved carrier | [§5.2](part_ii_carriers.md#52-drives) |
| operating point, round | [§16](part_v_execution.md#16-the-operating-point) |
| operating-point variable | [§11](part_iv_behavior.md#11-analog) rule 6 |
| optional parameters | [§3.2.7](part_i_values.md#32-parameters-and-calls) |
| panic | [§1](part_i_values.md#1-value-types) rule 11 |
| param, structural param | [§8.1](part_iii_structure.md#81-ports-and-params) |
| parse, elaboration, run, host (phases) | [§0.3](#03-conformance) |
| port, value input | [§8.1](part_iii_structure.md#81-ports-and-params) |
| probe | [§6](part_ii_carriers.md#6-continuous-carriers) |
| quantity | [§9](part_iii_structure.md#9-quantities) |
| reaction | [§12.1](part_iv_behavior.md#121-reactions) |
| record, stream, site, trace, reserved stream | [§19](part_vi_io.md#19-streams-and-checks) |
| `Repr` | [§2](part_i_values.md#2-repr) |
| result (of a module) | [§8](part_iii_structure.md#8-modules) rule 3 |
| scheduled time | [§17.2.2](part_v_execution.md#172-steps) |
| settle phase, settle commit | [§15](part_v_execution.md#15-time-and-the-instant) |
| signal-flow node | [§6](part_ii_carriers.md#6-continuous-carriers) rule 2 |
| small-signal rule | [§11.1.4](part_iv_behavior.md#111-one-block-every-analysis) |
| solve | [§15](part_v_execution.md#15-time-and-the-instant) |
| standard library | [§22](part_vii_stdlib.md#22-standard-library) |
| state (of the jump rule) | [§15.2.1](part_v_execution.md#152-the-jump) |
| sweep point | [§16.1](part_v_execution.md#161-analyses-without-time) |
| switch branch | [§11](part_iv_behavior.md#11-analog) rule 2 |
| template | [§8.4](part_iii_structure.md#84-templates-and-swappable-modules) |
| tick, time resolution | [§15](part_v_execution.md#15-time-and-the-instant) T1 |
| time point | [§15](part_v_execution.md#15-time-and-the-instant) |
| transport delay | [§12.1.4](part_iv_behavior.md#121-reactions) |
| value, value type | [Part I](part_i_values.md) introduction, [§1](part_i_values.md#1-value-types) |
| view (of a bundle) | [§7](part_ii_carriers.md#7-bundles) |

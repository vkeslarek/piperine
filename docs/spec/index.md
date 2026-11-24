# Piperine mixed-signal language: specification

This document specifies the Piperine mixed-signal language (MSL). It is the entry point of the
specification: scope, notation, conformance terms, the concept table, the file map and the
glossary index.

## 0. Concepts

### 0.1 Scope

MSL is a language for mixed-signal models. It describes digital, analog and mixed circuits for
simulation and optimization. MSL is a language based on Rust: its value language (types,
expressions, patterns, items, generics) takes Rust's design, and Rust's designers are credited
for it. MSL is not Rust, and this specification is complete by itself: every construct of the
language is defined here ([§0.4](#04-a-complete-specification)).

1. Every outcome of a simulation is a function of the source and of the host's actions alone.
   There are no races ([§12.1](part_iv_behavior.md#121-reactions)).
2. A model reuses existing models through `extern` modules: Verilog-A compiled to OSDI, and
   other simulators by cosimulation ([§8.5](part_iii_structure.md#85-run-settings-and-extern-modules)).
3. A bench, the top module that places a design with its stimulus and its monitors, is
   written in MSL. Sweeps, Monte Carlo, optimization and regression live in the host, a
   program outside the language ([§23](part_viii_host.md#23-testbenches-and-optimization)).
4. What is out of scope, and every open question, is listed in
   [Appendix G](appendix_g_scope.md). An open item has an id `G.n`.

Two things are deliberately not concepts. Output is one statement, `emit!`
([§19](part_vi_io.md#19-streams-and-checks)): no model reads it back. The host is outside the
language: it sets params, drives the top ports and reads everything.

### 0.2 Notation

**Section numbers.** Sections are numbered §1 … §23 across Parts I–VIII. Numbers are stable
across files. Appendices are lettered A–H; their items are cited as `A.2`, `B.3`, `G.6`.

**Rule numbers.** One convention holds in every file.

| Form | Cites |
| --- | --- |
| `§8.2.3` | rule 3 of subsection 8.2 |
| `§11 rule 3` | rule 3 of a top-level section; `§11.3` would name a subsection |
| `§15 step 3.3`, `§16 step 2` | a step of the procedure of §15 or §16 |
| `§15 T2`, `§15 I3`, `§15 S1`, `§16 O2`, `§16 H4`, `§17.2 A3` | an item of a lettered list of [Part V](part_v_execution.md): ticks, invariants, settle commit, operating point, held nodes, advance |
| `§22.2.12 rule 1` | a rule of a standard-library entry |
| `A.2 rule 3`, `B.3.3 item 4` | a rule or item of an appendix |

Rule numbers are identifiers. Within a section the rules are grouped under *Static semantics*
and *Dynamic semantics*, so they need not appear in ascending order. Each rule appears once.

**Code font.** `monospace` marks source text: keywords, operators, names, literals and
grammar symbols. An example block is MSL source unless it is labelled otherwise.

**Defined terms.** A term is set in **bold** at its single defining occurrence. Every other
use refers to that definition. [§0.9](#09-glossary-index) lists where each term is defined.

**Grammar.** Syntax is given in EBNF. The complete grammar of the language is
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
`attrs` belong to the value grammar. Their productions are
[Appendix B.3.7](appendix_b_grammar.md#b37-the-value-grammar), and their meaning is
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

### 0.4 A complete specification

**This specification defines the whole language.** A form, a type, a name or a method that it
does not define is not part of the language.

1. The value language is defined here in full: the lexical structure
   ([Appendix A](appendix_a_lexical.md)), the types, expressions, patterns, statements and
   control flow ([§1.1](part_i_values.md#11-scalar-types) to
   [§1.8](part_i_values.md#18-control-flow)), the items, traits, generics and names
   ([§3](part_i_values.md#3-functions) to [§3.9](part_i_values.md#39-constants)), and
   the grammar ([Appendix B.3](appendix_b_grammar.md)). The mixed-signal constructs are defined
   in Parts II to VIII.
2. The grammar of [Appendix B.3](appendix_b_grammar.md) is complete. Source text that no
   production derives is a parse error.
   [Appendix B.3.6](appendix_b_grammar.md#b36-forms-that-are-not-part-of-the-language) names
   the forms a front end reports with a code of their own.
3. The lexer and the grammar are the language's own ([Appendix A](appendix_a_lexical.md)). A
   conforming implementation need not accept a program of any other language.
4. The library is closed too: the names a model may use without declaring them are the catalogue
   of [Appendix H](appendix_h_library.md), the core names
   ([§3.4.2](part_i_values.md#34-files-and-imports)) and the standard library
   ([§22](part_vii_stdlib.md#22-standard-library)). A method, trait or type that none of them
   defines does not resolve (`E0161`).
5. No rule of this specification takes its meaning from the definition of another language. A
   mention of Rust is informative: it tells a reader who knows Rust what to expect.

> **Note (informative): relationship to Rust.** MSL is based on Rust. The value language keeps
> Rust's syntax and its design for types, expressions, patterns, items, traits and generics,
> so most value code reads the same in both. It does not keep Rust's references, ownership
> or mutability markers: MSL data are values. The table lists where MSL deliberately differs.
> It is a reading aid; the rules are those of the sections it cites.

| Topic | Rust | MSL | Stated in |
| --- | --- | --- | --- |
| integer types | widths 8, 16, 32, 64, 128 and the pointer width | any width `N >= 1` (`UInt<N>`, `Int<N>`, `u1` … `u128`); `usize` and `isize` are 64 bits everywhere | [§1.1](part_i_values.md#11-scalar-types) |
| integer overflow | a panic in a debug build, a wrap in a release build | wraps modulo `2^N` in every build | [§1](part_i_values.md#1-value-types) rule 6 |
| shift by the width or more | a panic or a masked amount | the mathematical result, truncated to the width | [§1.5](part_i_values.md#15-expressions) |
| unary `-` on an unsigned integer | rejected | accepted, wraps | [§1.5](part_i_values.md#15-expressions) |
| `char`, byte literals, byte strings | part of the language | not in the language; a string is the value type `str`, the text of a literal | [§1.2](part_i_values.md#12-composite-types), [A.8](appendix_a_lexical.md#a8-literals) |
| references | shared and mutable, with lifetimes | none: data are values, and hardware is named by identity through ports and handles | [§1](part_i_values.md#1-value-types) rule 2, [§1.4.7](part_i_values.md#14-typing-inference-and-conversion) |
| ownership | a value is moved unless its type is `Copy`; `Clone` duplicates | every value is copied where it is used; there is no `Copy`, no `Clone` and no move | [§1.4.6](part_i_values.md#14-typing-inference-and-conversion) |
| mutability | `let` and `let mut` | one `let`; every local may be assigned again, no parameter may | [§1.5.12](part_i_values.md#15-expressions) |
| slices | `[T]`, behind a reference | none: an array `[T; N]`, with `N` a const parameter where the length varies | [§1.2.5](part_i_values.md#12-composite-types) |
| heap, `unsafe`, threads, `async`, `dyn` | part of the language | not in the language | [§1](part_i_values.md#1-value-types) rule 2 |
| macros | definitions and invocations | none; `emit!` and `include_str!` are grammar forms | [A.1](appendix_a_lexical.md#a1-source-text-and-tokens) rule 4 |
| attributes | an open set | a closed set of five | [A.1](appendix_a_lexical.md#a1-source-text-and-tokens) |
| calls | positional | positional, plus optional trailing `Option` parameters and named arguments | [§3.2](part_i_values.md#32-parameters-and-calls) |
| implicit conversions | several coercions | one: `T` to `Option<T>` in an argument | [§3.2.3](part_i_values.md#32-parameters-and-calls) |
| `where` | trait bounds | a boolean const expression; bounds are written in the generic list | [§3](part_i_values.md#3-functions) rule 8 |
| receivers | `self`, `&self`, `&mut self` | `self`, a copy; a method returns what it changed | [§3.1.10](part_i_values.md#31-impl) |
| type aliases, associated types, `static`, `union`, `impl Trait`, qualified paths, restricted `pub`, loop labels | part of the language | not in the language | [B.3.6](appendix_b_grammar.md#b36-forms-that-are-not-part-of-the-language) |
| default values | `bool`, the integers and the floats implement `Default` | they do not: a carrier of one is given its first value; `Option<T>` and `Logic<N>` have defaults | [§3.3.4](part_i_values.md#33-uncertainty-and-absence), [H.8](appendix_h_library.md#h8-traits) |
| numeric literals | type suffixes | type suffixes and SI suffixes (`1k`, `2.2u`) | [A.2](appendix_a_lexical.md#a2-si-suffixed-reals) |
| library | `core`, `alloc`, `std` | the closed catalogue of Appendix H and the MSL standard library | [Appendix H](appendix_h_library.md), [§22](part_vii_stdlib.md#22-standard-library) |
| packages | Cargo, registries | `Piperine.toml`, `path` and `git` dependencies only | [§3.6](part_i_values.md#36-packages) |

### 0.5 The eleven concepts

The language has eleven concepts. Each has one rule. The "Covers" column lists what that rule
decides. Nothing with a rule of its own is left out of the table.

| # | Concept | Rule | Covers | § |
| --- | --- | --- | --- | --- |
| 1 | value and `fn` | data with no time, each level `false` or `true`; a function keeps no state, writes no carrier and places no instance | `Repr`, `Logic`, `Option`, closures, `Event`-returning functions | [1](part_i_values.md#1-value-types), [2](part_i_values.md#2-repr), [3](part_i_values.md#3-functions) |
| 2 | nature | says what a carrier holds and how its drives combine | `Bit` (one drive), resolved natures (`impl Resolve`), continuous natures (KCL) | [4](part_ii_carriers.md#4-natures) |
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
module Front(vin: Electrical, in clk: Bit, out q: Bit = false) {
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
| [`index.md`](index.md) | scope, notation, conformance terms, the concept table, who writes what, reading guide, file map, glossary index, the rules formerly inherited | §0 | — |
| [`part_i_values.md`](part_i_values.md) | the value language: types, expressions, patterns, statements, control flow, `Repr`, functions, `impl`, calls, uncertainty and absence, files, crates, packages, traits and generics | §1–3 | `E01xx` |
| [`part_ii_carriers.md`](part_ii_carriers.md) | natures, discrete carriers, drives, history, continuous carriers, bundles | §4–7 | `E02xx` |
| [`part_iii_structure.md`](part_iii_structure.md) | modules, ports, params, the body, instances in expressions, templates, `extern` modules, quantities | §8–9 | `E03xx` |
| [`part_iv_behavior.md`](part_iv_behavior.md) | behavior blocks, `analog`, `digital`, events, core primitives | §10–14 | `E04xx` |
| [`part_v_execution.md`](part_v_execution.md) | the instant, the jump, the operating point, analyses, transient, limiting | §15–18 | `E05xx` |
| [`part_vi_io.md`](part_vi_io.md) | streams and checks, input data, metadata | §19–21 | `E06xx` |
| [`part_vii_stdlib.md`](part_vii_stdlib.md) | the standard library | §22 | `E07xx` |
| [`part_viii_host.md`](part_viii_host.md) | testbenches, optimization, gradients across events | §23 | `E08xx` |
| [`appendix_a_lexical.md`](appendix_a_lexical.md) | lexical structure: source text, identifiers, keywords, literals, comments, punctuation | App. A | `E09xx` |
| [`appendix_b_grammar.md`](appendix_b_grammar.md) | B.1 the mixed-signal constructs, B.2 placement matrix, B.3 the complete grammar | App. B | — |
| [`appendix_c_implementation.md`](appendix_c_implementation.md) | lowering to AIR, the reference implementation | App. C | — |
| [`appendix_d_gallery_analog.md`](appendix_d_gallery_analog.md) | gallery: sources, clocks and continuous time | App. D | — |
| [`appendix_e_gallery_digital.md`](appendix_e_gallery_digital.md) | gallery: digital | App. E | — |
| [`appendix_f_gallery_mixed.md`](appendix_f_gallery_mixed.md) | gallery: mixed-signal systems | App. F | — |
| [`appendix_g_scope.md`](appendix_g_scope.md) | out of scope, and every open item `G.n` | App. G | — |
| [`appendix_h_library.md`](appendix_h_library.md) | the closed catalogue of the value library, each entry with its semantics | App. H | — |

### 0.9 Glossary index

Each term has one defining occurrence. The table points at it.

| Term | Defined in |
| --- | --- |
| accepted step, rejected step | [§17.2](part_v_execution.md#172-steps) |
| added keywords | [A.1](appendix_a_lexical.md#a1-source-text-and-tokens) |
| active (of a `limit` instance) | [§18](part_v_execution.md#18-limiting) rule 5 |
| `analog` block | [§11](part_iv_behavior.md#11-analog) |
| analysis, `Analysis` | [§11.1](part_iv_behavior.md#111-one-block-every-analysis) |
| behavior block | [§10](part_iv_behavior.md#10-behavior-blocks) |
| bench | [§23](part_viii_host.md#23-testbenches-and-optimization) |
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
| const expression | [§3.9.2](part_i_values.md#39-constants) |
| continued transient | [§16.4.1](part_v_execution.md#164-from-one-analysis-to-the-next) |
| contribution | [§11](part_iv_behavior.md#11-analog) |
| copy | [§1.4.6](part_i_values.md#14-typing-inference-and-conversion) |
| converged | [§17.1](part_v_execution.md#171-convergence) |
| converter | [§22.4](part_vii_stdlib.md#224-converters) |
| core primitive | [§14](part_iv_behavior.md#14-core-primitives) |
| crate, crate root | [§3.5](part_i_values.md#35-crates) |
| definition, delayed definition, anonymous definition | [§8.2](part_iii_structure.md#82-the-body) |
| `digital` block | [§12](part_iv_behavior.md#12-digital) |
| dependency | [§3.6](part_i_values.md#36-packages) |
| dependency name | [§3.5](part_i_values.md#35-crates) |
| direct dependence | [§8.3.5](part_iii_structure.md#83-instances-inside-expressions) |
| discrete carrier | [§5](part_ii_carriers.md#5-discrete-carriers) |
| discrete expression, continuous expression | [§8.2](part_iii_structure.md#82-the-body) |
| discrete nature, continuous nature | [§4](part_ii_carriers.md#4-natures) |
| drive | [§5.2](part_ii_carriers.md#52-drives) |
| due, pending | [§15](part_v_execution.md#15-time-and-the-instant) |
| effective multiplicity | [§8.5.3](part_iii_structure.md#85-run-settings-and-extern-modules) |
| equilibrium solve, transient step, jump solve, small-signal sweep | [§15](part_v_execution.md#15-time-and-the-instant), "Solves" |
| event position, guard | [§13](part_iv_behavior.md#13-events) |
| event time | [§15](part_v_execution.md#15-time-and-the-instant) |
| event, `Event` | [§13](part_iv_behavior.md#13-events) |
| extern module, implicit param | [§8.5](part_iii_structure.md#85-run-settings-and-extern-modules) |
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
| identity | [§1.4.7](part_i_values.md#14-typing-inference-and-conversion) |
| inertial delay | [§22.5.8](part_vii_stdlib.md#2258-inertial) |
| instance, placement | [§8.3](part_iii_structure.md#83-instances-inside-expressions) |
| instant, microstep | [§15](part_v_execution.md#15-time-and-the-instant) |
| irrefutable pattern, refutable pattern | [§1.6.11](part_i_values.md#16-patterns) |
| jump, right value | [§15.2](part_v_execution.md#152-the-jump) |
| left value | [§15.2](part_v_execution.md#152-the-jump) |
| level | [§2](part_i_values.md#2-repr) |
| limiting | [§18](part_v_execution.md#18-limiting) |
| located crossing | [§17.3](part_v_execution.md#173-locating-crossings) |
| lockfile | [§3.6](part_i_values.md#36-packages) |
| loop, relaxation | [§8.2.3](part_iii_structure.md#82-the-body) |
| manifest | [§3.6](part_i_values.md#36-packages) |
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
| package | [§3.6](part_i_values.md#36-packages) |
| panic | [§1](part_i_values.md#1-value-types) rule 11 |
| place, local | [§1.5.12](part_i_values.md#15-expressions) |
| param, structural param | [§8.1](part_iii_structure.md#81-ports-and-params) |
| package, parse, elaboration, run, host (phases) | [§0.3](#03-conformance) |
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
| uncertainty, absence, `Logic`, `Option`, initial value | [§3.3](part_i_values.md#33-uncertainty-and-absence) |
| value, value type | [Part I](part_i_values.md) introduction, [§1](part_i_values.md#1-value-types) |
| view (of a bundle) | [§7](part_ii_carriers.md#7-bundles) |

### 0.10 Rules formerly inherited, now stated

This table is informative. Earlier revisions left the value language to Rust's definition
([G.70](appendix_g_scope.md)). The rules below are those where writing the rule down meant
choosing it: the design never decided them, or the examples and the reference implementation
had settled them without a rule. Each row gives the rule now in force and where it stands. A
row marked *same* writes down what Rust does; a row marked *differs* does not, and says why.

| # | Question | Rule written | Stated in | Against Rust |
| --- | --- | --- | --- | --- |
| 1 | Type of a literal without a suffix | the type its position requires; otherwise `i32` for an integer and `f64` for a float | [§1.4.2](part_i_values.md#14-typing-inference-and-conversion) | same |
| 2 | Shift by the width or more, or by a negative amount | the mathematical result truncated to the width (0, or `-1` for a negative `Int`); a negative amount panics | [§1.5.21](part_i_values.md#15-expressions) | differs: no overflow error, following §1 rule 6 |
| 3 | `as` from a float to an integer | truncates toward zero, clamps to the range, NaN gives 0 | [§1.4.5](part_i_values.md#14-typing-inference-and-conversion) | same, extended to every width |
| 4 | Unary `-` on `UInt<N>` | accepted; `0 - x` modulo `2^N` | [§1.5.20](part_i_values.md#15-expressions) | differs: every integer operator wraps |
| 5 | Integer `/` and `%` | truncate toward zero; the remainder has the sign of the dividend; a zero divisor panics; the least value by `-1` wraps | [§1.5.19](part_i_values.md#15-expressions) | same, but the last case wraps where Rust panics |
| 6 | Type of an index | any integer type; out of range, a negative index included, panics | [§1.5.9](part_i_values.md#15-expressions), [§1.5.25](part_i_values.md#15-expressions) | differs: `mem[addr]` indexes with a `u8` ([E.5](appendix_e_gallery_digital.md)) |
| 7 | Order of evaluation | left to right; an assignment evaluates its right operand first | [§1.5.18](part_i_values.md#15-expressions) | same |
| 8 | Using a value twice | every value is copied where it is used; nothing is moved | [§1.4.6](part_i_values.md#14-typing-inference-and-conversion) | differs: Rust moves a value whose type is not `Copy`. Earlier revisions did the same and named this rule as the alternative ([G.71](appendix_g_scope.md)) |
| 9 | Reading through a borrow | there is no borrow: a parameter, an element and a result are values | [§1.2.6](part_i_values.md#12-composite-types) | differs: no references |
| 10 | A run of elements of any length | an array `[T; N]` with a const parameter `N`; there is no slice | [§1.2.5](part_i_values.md#12-composite-types) | differs: no `[T]` |
| 11 | Float arithmetic | IEEE 754, round to nearest even, no traps; the bits of a NaN are implementation-defined | [§1.1.7](part_i_values.md#11-scalar-types), [§1.1.8](part_i_values.md#11-scalar-types), [G.69](appendix_g_scope.md) | same |
| 12 | Accuracy of `exp`, `sin`, `powf` and the other elementary functions | within one unit in the last place, implementation-defined within it, one result per argument in a session | [H.2](appendix_h_library.md) rule 6, [G.69](appendix_g_scope.md) | stricter: Rust gives no bound |
| 13 | Float corner cases of the library | `round` ties away from zero; `min` and `max` ignore one NaN; float `%` is the truncated remainder; `clamp` panics on crossed or NaN bounds | [H.2](appendix_h_library.md), [§1.5.22](part_i_values.md#15-expressions) | same |
| 14 | Comparison | does not chain; floats by IEEE 754; tuples and arrays lexicographic; `None` below `Some` | [§1.5.5](part_i_values.md#15-expressions), [§1.5.23](part_i_values.md#15-expressions) | same |
| 15 | Scalar types have associated constants (`u8::MAX`) | no; a bound is written as a literal | [§1.1.5](part_i_values.md#11-scalar-types) | differs: follows from the closed catalogue |
| 16 | Which pattern forms exist | wildcard, binding (with `@`), literal, range, path, tuple, tuple struct, struct, array, or-pattern| [§1.6](part_i_values.md#16-patterns) | same forms, less `..hi`, `mut`, `ref` and `&p` (B.3.6) |
| 17 | Exhaustiveness of `match` | required; an arm with a guard does not count; an unreachable arm is not an error | [§1.8.6](part_i_values.md#18-control-flow) | same; there are no lints |
| 18 | `let … else`, `if let`, `while let` | kept; a `let` condition is the whole condition, never an operand of `&&` | [§1.7.6](part_i_values.md#17-statements-and-blocks), [§1.8.2](part_i_values.md#18-control-flow) | differs: no `let` chains |
| 19 | `break`, `continue`, `return` | innermost loop, no labels; `break value` in `loop` only; `return` in a function or closure only | [§1.8.10](part_i_values.md#18-control-flow), [§1.8.11](part_i_values.md#18-control-flow) | same, less labels (B.3.6) |
| 20 | The `?` operator | on `Option` only, in a function or closure that returns an `Option` | [§1.5.15](part_i_values.md#15-expressions) | narrower: there is no `Result` |
| 21 | Closures | capture by value where written; there is no `move` keyword; no recursion | [§1.5.14](part_i_values.md#15-expressions) | differs: no capture by reference |
| 22 | Type inference | local to one body; `_` only in a `let`, a closure parameter and the generic arguments of an expression | [§1.4.4](part_i_values.md#14-typing-inference-and-conversion), [§1.2.12](part_i_values.md#12-composite-types) | same |
| 23 | Enum discriminants | on enums without fields only; a missing one is the previous plus one; `as` yields it | [§1.3.7](part_i_values.md#13-structs-and-enums), [§1.4.5](part_i_values.md#14-typing-inference-and-conversion) | same |
| 24 | A type that contains itself | an error: there is no indirection | [§1.3.8](part_i_values.md#13-structs-and-enums) | same, with no `Box` to escape through |
| 25 | `for` over what | an integer range, an array by value, an iterator of H.7 | [§1.8.9](part_i_values.md#18-control-flow) | narrower: there is no `for x in &a`; `for x in a` reads the elements |
| 26 | Strings | `str` is a value type, the text of a literal; compared with `==` and `!=`; no other operation outside intrinsics | [§1.2.9](part_i_values.md#12-composite-types) | narrower, and a value where Rust has a reference |
| 27 | Traits | functions and constants, defaults, supertraits; no associated type; one `impl` per trait and type; an `impl` lives in the crate of the trait or of the type | [§3.7](part_i_values.md#37-traits) | same rules of coherence, stated without the exceptions Rust has |
| 28 | Method lookup on a value | the type's own `impl`s, then Appendix H, then the traits in scope | [§1.5.10](part_i_values.md#15-expressions), [§3.7.8](part_i_values.md#37-traits) | same; two traits with one method stay [G.67](appendix_g_scope.md) |
| 29 | When a generic body is checked | at each instantiation; a bound is checked against each argument | [§3.8.7](part_i_values.md#38-generics-and-bounds), [§3.8.9](part_i_values.md#38-generics-and-bounds) | differs: Rust checks the body once, against the bounds; follows §3 rule 9 and §4 rule 11 |
| 30 | What a const expression is | defined; only a `const fn` is callable in one | [§3.9.2](part_i_values.md#39-constants) to [§3.9.4](part_i_values.md#39-constants) | same, with a wider set of forms (loops, closures, floats) |
| 31 | Visibility | private to the declaring namespace and those nested in it, or `pub`; fields one by one; items visible whatever their order | [§3.4.13](part_i_values.md#34-files-and-imports), [§3.4.14](part_i_values.md#34-files-and-imports) | same, less `pub(…)` |
| 32 | Where `mod name;` finds its file | beside the root or a `mod.msl`; otherwise in the directory named after the declaring file | [§3.6.2](part_i_values.md#36-packages) | same |
| 33 | Identifiers | Unicode `XID_START`/`XID_CONTINUE`; compared by code points, no normalization | [A.7](appendix_a_lexical.md#a7-identifiers) rule 4 | differs: Rust normalizes to NFC |
| 34 | Literal forms | lower-case `0x`, `0o`, `0b`; float suffixes `f32` and `f64` only; the escapes of A.8 rule 11; raw strings | [A.8](appendix_a_lexical.md#a8-literals) | same, less `char` and byte forms |
| 35 | Reserved words | the 23 words of the last row of the keyword table | [A.1](appendix_a_lexical.md#a1-source-text-and-tokens) | same set as Rust's unused and reserved keywords, plus `static`, `type`, `unsafe`, `async`, `await`, `dyn`, `mut`, `ref`, `move` |
| 36 | What the library traits provide | `Default` gives `T::default()`, and no primitive numeric type implements it (§3.3.4); `Eq`, `Ord`, `Debug` and `Hash` give nothing a model calls; there is no `Clone` and no `Copy`; an enum does not derive `Default` | [H.8](appendix_h_library.md) | narrower. `default` has no row of its own in H.1 to H.7: confirm that it is callable |
| 37 | Iterators | adaptors are lazy; `rev` on arrays, integer ranges and `map`/`enumerate`/`zip`/`rev` of them; `sum` wraps for integers | [H.7](appendix_h_library.md) | same |
| 38 | Codes for the conditions that had none | `E0180` type mismatch, `E0181` inference, `E0182` `as`, `E0183` assignment, `E0184` patterns, `E0185` control flow, `E0186` structs and enums, `E0187` traits and generic arguments, `E0188` const expressions; all of phase elaboration | Part I | new codes; the reference implementation reports some of these under provisional codes |
| 39 | Which names may be assigned | every local, with no marker; no parameter, `self` included, and no name a closure captures | [§1.5.12](part_i_values.md#15-expressions), [§3.1.10](part_i_values.md#31-impl) | differs: Rust asks for `mut`, and allows it on a parameter |

# Piperine Roadmap

## Goal

**Piperine's expressiveness must equal or exceed ngspice's.** Anything you can
describe, stimulate, measure, or control in an ngspice deck + `.control` script,
you should be able to express in Piperine — usually more cleanly, because the
procedural layer is a real language, not a command shell.

This document is the single plan of record for *what we still support*. The
ngspice surface is catalogued across the `NGSPICE_*.md` reference docs in this
folder; each roadmap item below points at the relevant one.

## Where we are

Done (Phases 1–5 + language Waves 1–3):

- **Devices** — all 49 ngspice components: R/C/L/K, V/I + every waveform
  (PULSE/SIN/EXP/PWL/SFFM/AM/TRNOISE/TRRANDOM/port), B/E/G/F/H controlled sources,
  D/Q/M/J/Z/VDMOS semiconductors, switches, transmission lines, subckt.
  (`NGSPICE_COMPONENTS.md`, `NGSPICE_WAVEFORMS.md`)
- **Analyses** — `op tran ac dc noise tf sens sens_ac pz disto pss sp`, returning
  typed result objects (`TranResult`, `AcResult`, …) with `Signal` measurement
  methods and a `Complex` stdlib type. (`NGSPICE_CONTROL.md §Analyses`)
- **Measurement** — `$meas` + 9 structured `$meas_*` helpers.
- **Probes** — `$V`, `$I`.
- **Language** — full procedural layer: `if/case/for/while/repeat/forever/foreach`,
  `break/continue/return`, `++`/`--`/compound assignment, brace blocks, user
  `function`s (recursion), arrays/queues (`'{…}`, `foreach`, methods), `inside`,
  math stdlib (`$sqrt`…`$clog2`), randomization (`$urandom`/`$dist_*`), typed
  results, `paramset`, `always @(step|above)`.
- **Introspection & in-run control (Phase 4)** — operating-point device access
  (`inst.gm` → `@dev[gm]`, auto-`.save`), `$op_param`/`$model_param`, full vector
  retrieval (`$get_vec`), differential probes, `$alter`/`$altermod`/`$alterparam`
  (re-run without re-elaboration), `$set_option`/`$set_temp`/`$set_tnom`, physical
  constants. (`NGSPICE_EXPRESSIONS.md`, `NGSPICE_CONTROL.md`)
- **First-class behavioral expressions (Phase 5)** — B/E/G sources take a real
  Piperine expression (`V(a)*V(b)+$sin(…)`), lowered to ngspice B-source syntax by
  the `expr_serializer`. (`NGSPICE_BEHAVIORAL.md`)
- **Verilog-A lowering** — device modules may use Piperine's extended analog syntax
  (`{}` blocks, `++`, compound assign); `va_emit` lowers them to standard VA before
  OpenVAF compiles to OSDI.

## Guiding principles

1. **We own the netlist.** Piperine generates the SPICE deck, so ngspice's
   netlist-authoring conveniences (conditionals, includes, parameter math) are
   things we *implement at elaboration time*, not features to forward verbatim.
2. **The interpreter beats the control shell.** ngspice `.control` scripting
   (its `if/while/repeat/dowhile`, `let`, `echo`, loops) exists because a SPICE
   deck is otherwise static. Piperine already has a superior procedural language,
   so we re-express those capabilities as system tasks/result objects, not as a
   second scripting layer.
3. **Typed results over raw vectors.** ngspice hands back untyped Nutmeg vectors;
   Piperine wraps them (`Signal`, `Complex`, result objects). New data access
   follows that pattern.
4. **No macro magic.** Data tables + plain helpers (see the `Element` device
   builder). New surface stays readable and reason-about-able.
5. **One obvious form per concept; no lying syntax.** Piperine is its own coherent
   language, not a "superset that tolerates everything." Prefer a single idiom
   (`{}` blocks, `'{}` array literals, `return`, `=`); keep redundant Verilog forms
   only as accepted aliases, and **remove or alias** constructs that *look* like
   Verilog but don't behave like it (no-wildcard `casex`/`casez`, silent-`void`
   `time`/`logic`). The umbrella categories (two domains, scalar-vs-handle, three
   parameter kinds) are the whole mental model — see `docs/lang/SPEC.md §0`.

## Explicitly out of scope (and why)

| ngspice feature | Why we skip it |
|-----------------|----------------|
| `.control` flow (`if/while/repeat/dowhile`, `goto`) | The interpreter already provides these, better. |
| Interactive debugging (`stop`, `trace`, `iplot`, `step`, `where`) | Piperine drives non-interactively; use `always @(step)` + asserts for runtime checks. |
| Nutmeg plotting (`plot`, `gnuplot`, `asciiplot`) | Out of band — Piperine emits data; plotting is a downstream concern. |
| `ngbehavior` compatibility modes (hspice/ps/…) | We author the deck ourselves; no foreign-dialect parsing needed. |
| `.spiceinit` / startup RC files | Configuration belongs to the Piperine runtime, not deck dialect. |
| Netlist `.if/.elseif/.endif` conditionals | Resolved at elaboration by ordinary `if` / parameters. |

These are deliberately *not* gaps — Piperine is more expressive by replacing them.

---

## Phase 4 — Circuit introspection & in-run control · **done**

Reading a circuit's *internal* state and changing it between runs is implemented:
op-point device/model params (`inst.gm`, `$op_param`, `$model_param`), full vector
retrieval (`$get_vec`), differential probes, `$alter`/`$altermod`/`$alterparam`,
`$set_option`/`$set_temp`/`$set_tnom`, physical constants. (`NGSPICE_EXPRESSIONS.md`,
`NGSPICE_CONTROL.md`)

Remaining stragglers (backlog):

| Feature | ngspice form | Piperine target | Ref |
|---------|--------------|-----------------|-----|
| Initial conditions / hints | `.ic`, `.nodeset` | `$set_ic(node, v)`, `$nodeset(...)` | `NGSPICE_NETLIST.md §.ic/.nodeset` |
| Solver `options` block sugar | `.options reltol=… method=gear` | declarative `options` block over `$set_option` | `NGSPICE_NETLIST.md §.options` |

## Phase 5 — Behavioral expression language · first-class expr **done**

B/E/G sources now take a real Piperine expression (`V(a)*V(b)+$sin(…)`) lowered to
ngspice B-source syntax by the `expr_serializer`. Remaining behavioral forms:

| Feature | ngspice form | Piperine target | Ref |
|---------|--------------|-----------------|-----|
| POLY sources | `E1 … POLY(2) …` | `poly(...)` helper or expansion | `NGSPICE_BEHAVIORAL.md §6` |
| Nonlinear R/C/L | `R1 … R='…'`, `C1 … Q='…'` | expression-valued passives | `NGSPICE_BEHAVIORAL.md §7` |
| Behavioral `.func` | `.func f(x)='…'` | reuse Piperine functions, lowered into B-source exprs | `NGSPICE_NETLIST.md §.func` |

## Phase 6 — Statistical / Monte Carlo

`$dist_*`/`$urandom` already exist (Wave 3). This phase builds the *workflow*:
parametric runs, per-run plot management, and result aggregation — the thing real
analog verification spends its time on.

| Feature | ngspice form | Piperine target | Ref |
|---------|--------------|-----------------|-----|
| Tolerance distributions | `agauss/gauss/aunif/unif/limit` in `.param` | helpers returning sampled values (already expressible via `$dist_*`; add the named forms) | `NGSPICE_STATISTICAL.md §1` |
| Seeded reproducible runs | `set rndseed=…` | `$srandom` (done) — document MC pattern | `NGSPICE_STATISTICAL.md §2` |
| MC sweep + plot management | `mc_runs`, per-run `tran#N` plots | loop + re-run + collect into `Result[]`; aggregate `.mean()/.sigma()/.yield()` | `NGSPICE_STATISTICAL.md §3` |
| Lot vs device tolerance | dual-stage tolerance | a tolerance helper distinguishing lot/device | `NGSPICE_STATISTICAL.md §4` |
| Corner sweeps | manual | typed corner/sweep config (struct + loop) | — |

### DataFrame — the data through-line

A typed, analysis-independent result container (`DataFrame`) underpins Phases 6–7
and the eventual data-analysis / PyO3 export story. Every analysis lowers its
`AnalysisResult` into a column-oriented, indexed frame; Monte-Carlo loops `concat`
into one long frame. The *type* is simple Rust; the **ergonomics** need specific
language features — string indexing `df["x"]`, operator overloading for vectorized
`Signal` math, slicing, and (later) lambdas / `with` clauses. Full design,
prerequisites, and build order in [DATAFRAME.md](DATAFRAME.md).

## Phase 7 — Data, files, frequency domain

| Feature | ngspice form | Piperine target | Ref |
|---------|--------------|-----------------|-----|
| All 16 `.meas` types | `.meas` FIND/WHEN/TRIG-TARG/DERIV/INTEG/PARAM/… | complete the `$meas_*` set | `NGSPICE_EXPRESSIONS.md §.meas` |
| FFT / PSD | `fft`, `psd`, `.four` | `$fft(signal)` → spectrum result; `Signal.fft()` | `NGSPICE_CONTROL.md §fft/psd` |
| Calculus on vectors | `deriv`, `integ` | `Signal.deriv()/.integral()` (integral done) | `NGSPICE_EXPRESSIONS.md §Signal` |
| File output | `wrdata`, `write` rawfile | `$fopen/$fdisplay/$fwrite/$fclose` (SV file I/O); `$write_raw` | `NGSPICE_CONTROL.md §wrdata/write` |
| Rawfile import | `load` | `$load_raw(path)` → result object | `NGSPICE_CONTROL.md §load` |
| Select saved vectors | `.save`, `.probe` | `$save(...)` to limit captured signals | `NGSPICE_NETLIST.md §.save` |

## Phase 8 — Libraries & integration

| Feature | ngspice form | Piperine target | Ref |
|---------|--------------|-----------------|-----|
| External SPICE libs/models | `.include`, `.lib name section` | a normal `` `include "x.lib" `` — see *pluggable include handlers* below | `NGSPICE_NETLIST.md §.include/.lib` |
| Global nets | `.global vdd gnd` | `global wire vdd;` module construct | `NGSPICE_NETLIST.md §.global` |
| Control-script params | `.csparam` | bridge interpreter values into the deck | `NGSPICE_NETLIST.md §.csparam` |
| Subckt parameters | `.subckt … params:` | parameterized module instances | `NGSPICE_NETLIST.md §.subckt` |

### Pluggable include handlers

`` `include `` should not be limited to `.ppr` source. A plugin must be able to
**register an include handler keyed by file type**, so that `` `include "x.lib" ``
or `` `include "models.mod" `` is dispatched to the ngspice plugin, which injects
the file as raw SPICE cards into the netlist (rather than parsing it as Piperine).

- Preprocessor/elaboration looks up a handler by extension (or content sniff);
  the default handler parses `.ppr`, the ngspice plugin registers `.lib/.mod/.cir/.sp`
  → raw-netlist injection (optionally honoring `.lib` *section* selection).
- This replaces the earlier `$include_spice(...)` task idea: foreign SPICE files
  ride the same `` `include `` directive the user already knows, with the plugin
  deciding how to consume them. Keeps one include concept, plugin-extensible.

### `subckt` is a second module

A SPICE `.subckt NAME ports… / .ends` **is, conceptually, a Piperine `module`** —
there is no separate "subckt" abstraction. The mapping:

```
.subckt buffer in out vdd vss      module buffer(in, out, vdd, vss);
  M1 out in vdd vdd pmos      →        pmos M1(.d(out), .g(in), .s(vdd), .b(vdd));
  …                                    …
.ends                              endmodule

X1 a b vdd 0 buffer            →   buffer X1(.in(a), .out(b), .vdd(vdd), .vss(0));
```

Consequences:
- An in-file `.subckt` lowers to a `module`; its `X` instance becomes an ordinary
  *named* module instantiation (SPICE positional ports are mapped to the module's
  declared port names, in order).
- An **external** subckt (defined only in an included `.lib`) is still a module
  instantiation, with *positional* connections (`TL072 X1(a, b, c);`) since the
  port names live in the not-yet-included library. The include handler injects the
  subckt definition so the instance resolves at elaboration.
- There is **no `subckt` device / `.subckt_name` nomenclature** in the language —
  a subcircuit is always a module. (The legacy `subckt` extern module is slated for
  removal once the include handler lands.)
- Subckt parameters (`params:`) map to module `#(.param(...))` overrides.

## Phase 9 — Language completeness (Wave 4+)

Round out SystemVerilog expressiveness so testbenches stay ergonomic at scale.
(Waves 1–3 are done; the items below are what remains.)

- Enum methods (`.name()`, `.first()`, `.next()`) and runtime struct field access.
- Associative arrays `int aa[string]` — named result sets / parameter dictionaries.
- `package` — shared constants and helpers across files.
- `typedef` polish, `$cast`, `$sformat` (write-to-var), more string methods.

Deliberately **not** planned (verification-framework scope, low analog value):
classes/OOP, concurrent SVA, covergroups, clocking blocks, fork/join, interfaces,
DPI — verification-framework constructs with little analog value.

### File/module options — Rust-style attributes (`#![...]`)

**Why.** Some behaviors are *policy*, not code: whether undeclared nets are an
error, what the ground net is called, which discipline a bare net defaults to,
how strict the elaborator is. Today these are hardcoded. Verilog expresses such
policy with backtick directives (`` `default_nettype none ``, `` `timescale ``) —
stateful, order-dependent, and easy to get wrong. We prefer Rust's model:
**scoped, declarative attributes** that say *what policy applies here*, with no
imperative ordering.

**What we're doing.** Add a Rust-style attribute for setting elaboration/language
options, scoped to a file (inner, `#![...]`) or to the next item (outer, `#[...]`):

```verilog
#![strict_nets]                 // every net must be declared; undeclared = error
#![ground = "0"]                // name of the global ground net
#![default_discipline(electrical)]

#[strict_nets]                  // applies only to the module that follows
module tb; … endmodule
```

This is **distinct from `$set_option(...)`** (Phase 4): `$set_option` is a *runtime*
call that configures the *simulator* (reltol, method, temp) during a testbench;
`#![...]` is a *compile-time* declaration that configures *Piperine's elaboration*
before anything runs. Two different layers, two different mechanisms.

**Initial option set** (extensible):
- `strict_nets` — require net declarations; catches floating-node typos (a
  misspelled net silently becomes a floating SPICE node — the classic bug). This is
  the `default_nettype none` equivalent discussed for net declarations.
- `ground = "<name>"` — override the canonical ground net (default `gnd` → `0`).
- `default_discipline(<disc>)` — discipline for bare nets.

**Decision — this is the *only* policy mechanism.** Piperine deliberately does **not**
adopt the Verilog/SystemVerilog approach for options:
- **No** backtick policy directives (`` `default_nettype ``, `` `default_discipline ``,
  `` `pragma ``, `` `timescale ``). They are stateful and order-dependent ("valid from
  here until the next one / `` `resetall ``") — a C-preprocessor model that is easy to
  get wrong and hard to reason about locally.
- **No** `(* attr = val *)` attribute instances for policy. (We still parse `(* *)`
  for inert tool metadata, but options do not ride it.)

One scoped, declarative attribute (`#![...]` file / `#[...]` item) expresses all
policy. The scope is lexical and obvious; there is no "from this line onward" state.

**Plugin-extensible.** Like include handlers, a plugin registers the option keys it
understands (the ngspice plugin owns simulator-policy keys), so the attribute set
grows without touching the core grammar.

**Mechanics.** Lexer learns `#![` / `#[` / `]` (note `#` already lexes as `Hash` for
`#(...)` param overrides — the attribute forms are a distinct token sequence). The
parser collects attributes into an options table consumed by the elaborator. Default
behavior is unchanged when no attribute is present, so existing `.ppr` files keep
working.

---

## ngspice coverage matrix

Status: ✅ done · 🚧 planned (phase) · ⛔ out of scope (interpreter/own-netlist replaces it)

| ngspice area | Status |
|--------------|--------|
| Components (R/C/L/K/V/I/B/E/G/F/H/D/Q/J/M/Z/VDMOS/switch/tline/subckt) | ✅ |
| Source waveforms (PULSE/SIN/EXP/PWL/SFFM/AM/TRNOISE/TRRANDOM/port) | ✅ |
| Analyses (op/dc/ac/tran/noise/tf/sens/disto/pz/pss/sp) | ✅ |
| Typed results + Signal + Complex | ✅ |
| `$meas` (core patterns) | ✅ / 🚧 P7 (all 16) |
| Randomization (`$urandom`, `$dist_*`, seed) | ✅ |
| `@device[param]` operating-point access (`inst.gm`) | ✅ |
| `alter`/`altermod`/`alterparam` | ✅ |
| `.options` / `.temp` (`$set_option`/`$set_temp`) | ✅ / 🚧 `.ic`/`.nodeset` P4 |
| Physical constants | ✅ |
| Full vector retrieval / differential probes | ✅ |
| B-source expression language (first-class) | ✅ |
| POLY / nonlinear R/C/L | 🚧 P5 |
| Monte Carlo workflow + aggregation | 🚧 P6 |
| `.param` distributions (agauss/gauss/…) | 🚧 P6 |
| FFT / PSD / `.four` | 🚧 P7 |
| File output / rawfile import | 🚧 P7 |
| `.save` / `.probe` | 🚧 P7 |
| `.include` / `.lib` interop | 🚧 P8 |
| `.global` nets | 🚧 P8 |
| `.csparam` / subckt params | 🚧 P8 |
| `.control` flow (if/while/repeat/dowhile) | ⛔ interpreter |
| Interactive debug (stop/trace/iplot/step/where) | ⛔ interpreter + `always` |
| Nutmeg plotting | ⛔ downstream |
| `ngbehavior` compat modes / `.spiceinit` | ⛔ own netlist |
| Netlist `.if/.elseif` conditionals | ⛔ elaboration-time `if` |

When every 🚧 row is ✅, Piperine meets the goal: a strict superset of ngspice's
expressiveness, in one coherent language.

---

## Backlog (under discussion)

### Replace `paramset` with `model` — an instantiation-shaped, inheritable model entity

**Decision (backlog):** remove `paramset` and introduce a `model` entity. Rationale:
`paramset` is a hidden-golden-rule construct (it secretly does two things — preset
params *and* emit a SPICE `.model`), and we dislike both keeping it and inventing a
second odd entity. `model` collapses it into one first-class thing shaped like the
device instantiation the user already knows.

**Shape — a model is "instantiated" like a device, with param overrides:**

```verilog
// model NAME = BASE #( overrides );
//   BASE is a device (sets the .model TYPE) or another model (inheritance).
model nmos_svt = nmos     #(.vth0(0.40), .tox(2e-9), .u0(450));
model nmos_lvt = nmos_svt #(.vth0(0.30));     // inherits nmos_svt, overrides vth0 only
```

- Same `#(.param(value))` override syntax as a device instance — no special
  `.x = y;` paramset grammar. One override mechanism across the language.
- **Inheritance:** a model may derive from another model; it starts from the base's
  params and overrides a few — exactly paramset's ergonomics, but explicit and
  layered (the "almost-inheritance" the user wants).
- Maps to ngspice `.model <name> <TYPE> (merged params)`; the TYPE comes from the
  root device. Instances reference it like a device variant
  (`nmos_svt #(.w(1u)) M1(...)`) or via `.model(nmos_svt)`.

**Open questions for the design doc:**
- Is `model` a top-level item or instantiation expression? (top-level, named.)
- Override resolution order for multi-level inheritance (base → … → leaf, leaf wins).
- Where the device→model param split lives (which params are `.model` card vs
  instance params): today the device knows its `spice_model_type`; `model` would
  carry only model-card params, instances carry instance params.
- Migration: rewrite the `paramset … endparamset` blocks (and `tools/spice2ppr.py`
  emission, and the ported examples) to `model NAME = BASE #(…);`.

When taken up: write `docs/development/MODELS.md` (full design), then implement
behind the elaborator's existing `.model` emission (paramset already proves the
lowering works — this is mostly a front-end/ergonomics change).

### Typed nets — disciplines / natures, multiple net & signal types

**Backlog:** make nets *typed* instead of untyped SPICE nodes. Today a net is an
implicit, type-less node and the Verilog-AMS `discipline`/`nature` declarations and
the digital net-type words (`wand`, `tri`, `supply0/1`, …) are **parse-tolerated but
inert** (SPEC §0.7–§0.8). Eventually give them meaning.

**What it buys (why it's worth doing):**
- **Dimensional analysis / unit checking** — a `nature` carries physical units and
  tolerances (Potential = V, Flow = A, `abstol`, …); a `discipline` (`electrical`,
  `thermal`, `rotational`, …) types a net to a physical domain. With them, the
  elaborator/interpreter can *check units* (no adding a voltage to a current) and
  pick per-domain access functions instead of hardcoding `V()`/`I()`.
- **Different net types & signals** — distinguish analog continuous nets from digital
  logic nets (mixed-signal), and electrical from non-electrical domains. The access
  function and the SPICE/behavioral lowering follow from the net's discipline.

**Sketch:**
```verilog
nature Voltage; units = "V"; abstol = 1e-6; endnature
nature Current; units = "A"; abstol = 1e-12; endnature
discipline electrical; potential Voltage; flow Current; enddiscipline

electrical a, b;          // typed nets — a discipline, not bare nodes
// V(a,b), I(branch) are derived from `electrical`, units checked
```

**Open questions:**
- How far to take it: just electrical + unit checking, or full multi-domain?
- Interaction with the (currently implicit, untyped) net model and `gnd → 0`.
- Whether digital/logic nets become a real (mixed-signal) thing or stay out of scope.
- Where unit checking runs (elaboration, since analog exprs are serialized there).

This activates the inert `discipline`/`nature`/net-type grammar that already parses;
a future `docs/development/NETS.md` would design it. Pairs naturally with the `#![...]`
options work (a strict mode could *require* a discipline per net).

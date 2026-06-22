# Piperine Roadmap

## Goal

**Piperine's expressiveness must equal or exceed ngspice's.** Anything you can
describe, stimulate, measure, or control in an ngspice deck + `.control` script,
you should be able to express in Piperine — usually more cleanly, because the
procedural layer is a real language, not a command shell.

This document is the single plan of record for *what we still support*. It
supersedes the per-phase sprint lists in `NGSPICE_INTEGRATION_PLAN.md` (kept for
historical reference). The ngspice surface is catalogued across the `NGSPICE_*.md`
reference docs in this folder; each roadmap item below points at the relevant one.

## Where we are

Done (Phases 1–3 + language Waves 1–3):

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

## Phase 4 — Circuit introspection & in-run control

The biggest expressiveness gap: reading a circuit's *internal* state and changing
it between runs. This is what turns a testbench from "stimulate + probe outputs"
into "characterize the device."

| Feature | ngspice form | Piperine target | Ref |
|---------|--------------|-----------------|-----|
| Operating-point device params | `@M1[gm]`, `@Q1[ic]`, `@D1[vd]` | `$op_param("M1","gm")` and/or `inst.gm` on a device handle | `NGSPICE_EXPRESSIONS.md §@device` |
| Model-param read | `@model[vth0]` | `$model_param(model, param)` | same |
| Full vector retrieval | `let v = v(out)` | `$get_vec("v(out)")` → `real[]` (whole sweep, not just last) | `NGSPICE_CONTROL.md §let/print` |
| Differential / formatted probes | `v(a,b)`, `vdb()`, `vp()` | `$V("a","b")`, `Signal.db()/.phase()` extensions | `NGSPICE_EXPRESSIONS.md` |
| Change params between runs | `alter`, `altermod`, `alterparam` | `$alter(inst, param, val)`, `$altermod`, `$alterparam` — re-run without re-elaboration | `NGSPICE_INTEGRATION_PLAN.md §3J` |
| Solver options | `.options reltol=… method=gear` | `$set_option(key, val)` / an `options` block | `NGSPICE_NETLIST.md §.options` |
| Temperature | `.temp`, `temp=` | `$set_temp(t)` / sweep | `NGSPICE_NETLIST.md §.temp` |
| Initial conditions / hints | `.ic`, `.nodeset` | `$set_ic(node, v)`, `$nodeset(...)` | `NGSPICE_NETLIST.md §.ic/.nodeset` |
| Physical constants | `kboltz`, `echarge`, `M_PI` | predefined identifiers / `$const(...)` | `NGSPICE_INTEGRATION_PLAN.md §3I` |

## Phase 5 — Behavioral expression language (first-class)

Today B/E/G sources take their expression as a **string** parameter. Make the
expression a real Piperine expression that compiles to ngspice's B-source syntax,
with `V(node)`, `I(branch)`, ternaries, and math — so behavioral modeling is
type-checked and composable.

| Feature | ngspice form | Piperine target | Ref |
|---------|--------------|-----------------|-----|
| First-class B-source expr | `B1 out 0 V=v(a)*v(b)+sin(...)` | `bsource_v #(.v( V(a)*V(b) + $sin(...) ))` — expr serializer | `NGSPICE_BEHAVIORAL.md §1` |
| Nonlinear E/G | `E1 … VOL='…'`, `G1 … CUR='…'` | behavioral forms of `vcvs`/`vccs` | `NGSPICE_BEHAVIORAL.md §2–3` |
| POLY sources | `E1 … POLY(2) …` | `poly(...)` helper or expansion | `NGSPICE_BEHAVIORAL.md §6` |
| Nonlinear R/C/L | `R1 … R='…'`, `C1 … Q='…'` | expression-valued passives | `NGSPICE_BEHAVIORAL.md §7` |
| Behavioral `.func` | `.func f(x)='…'` | reuse Piperine functions, lowered into B-source exprs | `NGSPICE_NETLIST.md §.func` |

(An `expr_serializer` already exists in `piperine-ngspice`; this phase makes it the
front door for behavioral sources.)

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
| Rawfile import | `load` | `$load_raw(path)` → result object | `NGSPICE_INTEGRATION_PLAN.md §5G` |
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
- An **external** subckt (defined only in an included `.lib`) has no Piperine
  `module` to bind to. Such an `X` keeps a positional reference — handled by the
  pluggable include handler injecting the subckt and emitting the `X` card. The
  generic `subckt` device remains only for these opaque external cases.
- Subckt parameters (`params:`) map to module `#(.param(...))` overrides.

## Phase 9 — Language completeness (Wave 4+)

Round out SystemVerilog expressiveness so testbenches stay ergonomic at scale.
(Tracked against `SYSTEMVERILOG_FEATURES.md`.)

- Enum methods (`.name()`, `.first()`, `.next()`) and runtime struct field access.
- Associative arrays `int aa[string]` — named result sets / parameter dictionaries.
- `package` — shared constants and helpers across files.
- `typedef` polish, `$cast`, `$sformat` (write-to-var), more string methods.

Deliberately **not** planned (verification-framework scope, low analog value):
classes/OOP, concurrent SVA, covergroups, clocking blocks, fork/join, interfaces,
DPI — see `SYSTEMVERILOG_FEATURES.md` rationale.

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
| `@device[param]` operating-point access | 🚧 P4 |
| `alter`/`altermod`/`alterparam` | 🚧 P4 |
| `.options` / `.temp` / `.ic` / `.nodeset` | 🚧 P4 |
| Physical constants | 🚧 P4 |
| Full vector retrieval / differential probes | 🚧 P4 |
| B-source expression language (first-class) | 🚧 P5 |
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

# ngspice netlists ported to Piperine

53 real ngspice/SPICE netlists mechanically translated to Piperine `.ppr` by
[`tools/spice2ppr.py`](../../tools/spice2ppr.py). They stress-test the language on
real analog circuits — guitar-pedal preamps, filters, rectifiers, amplifiers — and
double as a worked-example corpus.

## Sources (all MIT-licensed)

| Repo | What | Files here |
|------|------|------------|
| [thecowgoesmoo/SPICEyPedals](https://github.com/thecowgoesmoo/SPICEyPedals) © 2025 Richard Moore | Guitar-pedal & analog circuits | `*.cir` named after the pedal |
| [Kevin1289/skidl-spec2circuit](https://github.com/Kevin1289/skidl-spec2circuit) | Spec-to-circuit benchmark | `skidl_*` |
| [astorguy/learn_ngspice](https://github.com/astorguy/learn_ngspice) | ngspice tutorial circuits | `learn_*` |

Each file's header comments name its source netlist. These ports are derived works
of MIT-licensed originals; see each upstream repo for full license text.

## How the mapping works

| ngspice | Piperine |
|---------|----------|
| `Rx a b 1k` / `Cx`/`Lx` | `res #(.r(1000)) Rx(.p(a), .n(b));` etc. |
| node `0` | `gnd` |
| numeric node `5` | `n5` |
| `Vx a b DC 9 AC 1` | `vsource #(.dc(9), .acmag(1)) Vx(.p(a), .n(b));` |
| `Vx a b SIN(0 50m 500)` | `vsin #(.vo(0), .va(0.05), .freq(500)) Vx(...);` |
| `Dx a c MODEL` (model in-file) | `m_MODEL Dx(.a(a), .c(c));` (a paramset) |
| `Dx a c MODEL` (model via include) | `d #(.model("MODEL")) Dx(.a(a), .c(c));` |
| `.model NAME D (is=… n=…)` | `paramset m_NAME d; .model = "NAME"; .is = …; endparamset` |
| `.subckt NAME p… / .ends` | `module NAME(p…); … endmodule` — **a subckt is just a module** |
| `Xx a b NAME` (in-file subckt) | `NAME Xx(.p1(a), .p2(b));` (named instantiation) |
| `Xx a b NAME` (external subckt) | `NAME Xx(a, b);` (positional instantiation) |
| `.tran 2u 100m 80m` / `.op` / `.ac …` | `$tran(2e-6, 0.1, 0.08);` / `$op();` / `$ac(…);` in `initial` |
| `.control … .endc` | dropped — the `initial` block replaces the control script |
| `.include "x.lib"` | `` `include "x.lib" `` — see "planned features" |

SI suffixes are converted (`1meg`→`1e6`, `10n`→`1e-8`, `33k`→`33000`); ngspice
`{expr}` value braces are stripped.

## Planned-language features used

Where the source needs something Piperine doesn't implement yet, the translation
emits the **planned** syntax (per `docs/development/ROADMAP.md`):

- **SPICE model/lib includes** → a normal `` `include "x.lib" ``. Resolving a
  non-`.ppr` include is a *pluggable include handler* (ROADMAP Phase 8): the
  ngspice plugin will inject the file as raw netlist. Until then, the 19 files
  with such includes don't fully parse — they document the target syntax.

## Status

- **34 / 34** include-free files parse cleanly with the Piperine parser
  (re-check with `cargo run --example verify_ported -p piperine -- examples/ngspice-ported/<file>.ppr`).
- 19 files use the planned `` `include `` of external SPICE libs and will parse
  once the include-handler feature lands.

## Known approximations (mechanical transpile)

- **Behavioral B-sources** (`Bx … V={expr}`) and **switches** (`Sx`) are emitted as
  commented placeholders — first-class behavioral expressions are ROADMAP Phase 5.
- **External subckts** (opamps from included libs) are instantiated as modules
  with *positional* connections (`TL072 X1(a, b, c);`), since their port names live
  in the not-yet-inlined library. There is no `subckt` device nomenclature — a
  subcircuit is always a module.
- Model parameters that come from `.include`d libraries are not inlined.

These are limits of a generic transpiler, not of Piperine — a hand-port would use
idiomatic constructs (and, later, the planned features) directly.

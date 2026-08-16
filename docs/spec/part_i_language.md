# Part I — Language Specification

Piperine's sole frontend is Verilog-AMS (LRM 2.4). This Part is the normative
description of the language **as Piperine implements it**: which LRM semantics apply,
where Piperine's own pipeline narrows or extends them, and what parses but does not yet
reach a simulated device. It does not restate the grammar — Appendix B is the grammar
reference, anchored directly to `VAMS-LRM-2-4.md` Annex A. This Part is about meaning,
not productions.

Every claim below is grounded in `crates/piperine-lang/src/{ast,grammar,lexer.rs,
preprocessor.rs,parser.rs,keywords.rs}` (the parser, merged into `piperine-lang` at
MD-47 — see Part I §0) and `crates/piperine-lang/src/elab/` (the elaborator, Part II)
and cross-references the LRM clause each construct implements.

## §0 A note on the crate merge (MD-47)

Until 2026-08-13, the parser (`ast`/`grammar`/`lexer`/`preprocessor`/`parser`/`error`/
`fmt`/`keywords`) lived in a separate crate, `piperine-vams`, behind a mechanically
enforced zero-dependency purity gate (`tests/purity.rs`) — the boundary existed because
the parser was meant to have no elaboration/serde/diagnostics dependencies at all. That
boundary was retired the same day (MD-47, user decision): the purity gate is deleted,
not weakened, and the parser's modules now live physically inside `piperine-lang`
alongside the elaborator (`elab`/`pom`/`eval`/`annex_e`/`value`/`source_map`/`math`/
`vams_include`). Every `piperine_lang::…` import path is unaffected; only the five
downstream crates that used to depend on both crates had their now-redundant
`piperine-vams` dependency line removed. This Part cites source paths under
`crates/piperine-lang/src/` throughout — there is no `crates/piperine-vams/` on this
branch.

## Contents

- §1 Lexical conventions (LRM ch. 2)
- §2 Types: nature, discipline, net, parameter, genvar (LRM ch. 3)
- §3 Expressions (LRM ch. 4)
- §4 Analog behavior and contribution statements (LRM ch. 5)
- §5 Hierarchical structures and module instantiation (LRM ch. 6)
- §6 The digital subset: what's live vs. parse-only (LRM ch. 7, 8)
- §7 Functions and tasks (LRM §5.9, ch. 9)
- §8 Piperine's own attribute extension: `(* device *)`/`(* port *)`
- §9 Preprocessor (LRM Annex E)
- §10 What is explicitly not supported
- §11 Diagnostics and the fail-loud contract

---

## §1 Lexical conventions (LRM ch. 2)

`crates/piperine-lang/src/lexer.rs` implements LRM ch. 2's token set: identifiers
(escaped `\name ` and unescaped forms, §2.7), the four numeric literal forms (`Integer`,
`Real`, sized/based `Integer` per the LRM's `size'base digits` production, and
scaled-real `1.5n`/`2.2M`-style unit suffixes), string literals, and comments
(`//` line, `/* */` block). `crates/piperine-lang/src/keywords.rs` carries the reserved
word table Appendix B §B.1.2's `annex_b_keywords.rs` gate re-derives at test time from
the LRM text itself, so the reserved set cannot silently drift from what the standard
actually reserves. The lexical layer is otherwise unremarkable LRM ch. 2 — this Part
does not restate token grammar; Appendix B's B.1 conformance-gate description is the
authoritative pointer to how lexical/grammar coverage is checked.

### §1.1 Documentation comments (`///`) — a Piperine extension

The LRM knows two comment forms and assigns neither any meaning beyond "ignore this
text" (§2.3). Piperine **extends** the line-comment form: a run of consecutive `///`
lines immediately above a declaration is that declaration's documentation, carried
through elaboration onto the model object and surfaced by the language server as
Markdown on hover. An ordinary `//` comment is never documentation — the distinction is
the third slash and nothing else.

```verilog-ams
/// A two-terminal resistor.
/// Obeys Ohm's law; `r` is the resistance in ohms.
module Resistor (inout electrical p, inout electrical n);
    /// Resistance, in ohms. Must be positive.
    parameter real r = 1e3 from (0:inf);
    analog I(p, n) <+ V(p, n) / r;
endmodule
```

**Which declarations carry documentation.** `module`/`macromodule`, `parameter`/
`localparam`, port declarations, net (discipline) declarations, variable declarations
(`integer`/`real`/`time`/`realtime`), `nature`, and `discipline`. Each reaches a `doc:
Option<String>` field on its POM node (`crates/piperine-lang/src/pom/module.rs`,
`pom/nature.rs`), readable from either host through the accessors Part VIII describes
(`Module::doc()`, `Param::doc()`, …) — the editor and a programmatic consumer read the
same string. A discipline is re-exported as its surface AST node verbatim (Part IV's
design F1), so its documentation is read from `ast::DisciplineDecl` rather than a POM
mirror.

**Attachment rules**, all three mechanically gated by
`crates/piperine-lang/tests/vams_doc_comments.rs`:

- Consecutive `///` lines form **one** run, joined with newlines; each line's marker and
  surrounding whitespace are stripped. Blank lines between `///` lines do not break the
  run — an intervening ordinary `//` comment does.
- The run attaches to the declaration's **first token**, captured before anything is
  consumed. A doc comment above an attribute-instanced declaration therefore still
  attaches to it, because the `(*` of the `(* device = … *)` instance (§8) is that first
  token, not the `module` keyword.
- A run with no following declaration (a trailing `///` at end of file) is discarded, and
  an undocumented declaration keeps `doc: None` — documentation never leaks from one
  declaration onto the next.

**Where the text is carried.** The lexer already preserves comment text
(`Tok::LineComment`), but `grammar/cursor.rs` filters every comment out of the token
stream before the grammar sees it: each production in `grammar/{expr,stmt,item}.rs` is
written against a comment-free positional stream, and leaving comments in would force
every one of them to skip them by hand. `Grammar::new` therefore builds a **side table**
from the raw stream first — doc run → the buffer offset of the next meaningful token —
which `Grammar::doc_at` reads at the single point each declaration is constructed. Token
indexing is byte-identical to a parse with no doc comments in it. This is the same
derived-side-artifact shape `Design::flat_modules` (Part II) and `SourceProvenance`
(Part X) already use, for the same reason: the authored structure stays untouched and the
derived data rides alongside it.

Constructs with no authored declaration behind them are correctly undocumentable: the
implicit nets §3.7 creates from an instance connection, and implicit branches. Node kinds
whose AST nodes carry no `doc` field yet — `aliasparam`, `event`, instance labels,
`branch` — resolve to `None`; adding one is mechanical if they ever need hover
documentation.

## §2 Types: nature, discipline, net, parameter, genvar (LRM ch. 3)

**Nature** (§3.6.1) and **discipline** (§3.6.2) are top-level declarations, lowered by
`LowerDeclarations` (Part II §4) into `pom::Nature` and the AST's own `DisciplineDecl`
respectively (Part IV §5). A discipline binds a nature to `potential` and/or `flow`;
Piperine implements Annex F's **Basic** discipline-resolution method (§7.4.4.1) for
nets that declare no explicit discipline — see Part II §4's `ResolveDisciplines` entry
for the exact scope narrowing (single-module fixed point, not full hierarchy-wide
`connect...resolveto` resolution).

**Net** declarations (§3.6.3) name a discipline directly — `electrical p, n;` — no
PHDL-style value/net type lattice sits between the discipline name and the POM
`NetType::Discipline(String)` it lowers into (Part IV §4's callout on this point).
Net arrays (`NetType::Array`) are supported; width is the product of array dimensions.

**Parameter** declarations (§3.4) support the LRM's `from`/`exclude` range-constraint
clauses — violated at their declared value (including the parameter's own default)
fails loud as `ParamRangeViolation` (E4006, Part II §6). `paramset` (§6.4, a named
reusable parameter set bound to a module) is registered at the top level
(`Design::paramsets()`); `specparam`/`defparam` (§3.4/§6.4.2) parse
(`SpecparamDecl`/`Defparam` in the AST) but are **not lowered at all** — confirmed
directly against `elab_module`'s per-`ModuleItem` match in
`crates/piperine-lang/src/elab/lower/vams.rs`, where `ModuleItem::Specparam(_) |
ModuleItem::Defparam(_)` is one of a no-op arm's variants (§6 below has the full list
of that arm's members). A `specparam`/`defparam` in source parses and elaborates
without error, but contributes nothing to the POM.

**Genvar** (§3.5) — a `generate`-loop index, declared per module
(`Module::genvars()`); unrolled instances the loop produces are a **derived** artifact
(Part II §4's `UnrollGenerate` entry), never written back into the authored `genvar`
declaration or the module's own instance list.

**Branch** (§3.12/§5.4.1) — explicit (`branch (p,n) name;`) or implicit (created on an
access-function call site's first use). Covered fully in Part IV §5; the one semantic
rule worth restating here: a branch contributed as both a potential source and a flow
source is a fail-loud error (`BranchKindConflict`, E4001, LRM §5.4.2) *unless* it is a
switch branch (§5.6.5) — the one construct in the language explicitly permitted to
toggle kind at runtime through conditional contribution statements.

## §3 Expressions (LRM ch. 4)

`crates/piperine-lang/src/grammar/expr.rs`/`ast/expr.rs` implement LRM ch. 4's
expression grammar: the standard operator precedence table, ternary `?:`, concatenation
`{a, b, c}` (used both as a genuine bit-concatenation and, in Piperine's operator
lowering, as the coefficient-list carrier for `laplace_*`/`zi_*`'s numerator/denominator
arguments — VAMS has no array-literal syntax, so this is the LRM's own mechanism, not
a Piperine extension — see Part V §2), and the analog access-function call forms
(`V(a,b)`, `I(br)`). Every builtin name callable in expression position (math functions,
analog operators, `$`-system functions) resolves through the native tables Part V
documents — there is no `extern`-declared surface for any of them (Part V §0).

## §4 Analog behavior and contribution statements (LRM ch. 5)

An `analog` block (`BehaviorKind::Analog`), an `analog initial` block
(`AnalogInitial`, §5.7, runs once before the first analysis time point), and the
contribution statement `V(p,n) <+ expr;` / `I(br) <+ expr;` (§5.4) are the core of the
analog language surface, and the one area Annex C (§4 below) most tightly constrains.
`event_control` forms legal inside analog (`@(cross(...))`, `@(above(...))`,
`@(timer(...))`, `@(initial_step)`, `@(final_step)`) drive discrete state transitions
inside otherwise-continuous behavior — the thermostat hysteresis pattern
(`examples/07_thermostat.vams`) is the canonical worked example:

```verilogams
module Controller (inout electrical temp, inout electrical gnd);
    parameter real t_high = 25.0;
    parameter real t_low = 22.0;
    parameter real power = 1.0;
    real heater;
    analog begin
        @(initial_step) heater = 1.0;
        I(temp, gnd) <+ -heater * power;
        @(above(V(temp, gnd) - t_high)) heater = 0.0;
        @(above(t_low - V(temp, gnd))) heater = 1.0;
    end
endmodule
```

`heater` is Verilog-A module-scope persistent analog state — seeded once at
`initial_step`, flipped only by the `above` events, read continuously by the
contribution. Module-scope `real`s in an analog block are the direct VAMS equivalent of
what PHDL called a `var`; there is no separate `var` keyword in VAMS analog behavior.

**Annex C enforcement** (`Typecheck`, T27, Part II §4) is the pass that keeps a
`.vams` analog block inside the LRM's own restricted analog subset: non-blocking
assignment (`<=`), `@(posedge ...)`-style edge event control, and `casex` are all
illegal inside `analog` and fail loud citing "Annex C" — `crates/piperine-lang/tests/
vams_annex_c.rs` is the live regression suite for exactly these three rejections.

## §5 Hierarchical structures and module instantiation (LRM ch. 6)

`module`/`macromodule`/`connectmodule` declarations (Annex A.1.2) lower into POM
`Module`s (Part II §4's `ElabModules` entry). Instantiation supports both named
(`.port(net)`) and ordered port-connection styles and parameter overrides at the
instantiation site (`#(.param(value))`), exercised end-to-end by
`crates/piperine-lang/tests/vams_lower_structure.rs::hierarchy_is_navigable_and_never_
flattened` — the live proof that the authored `instance → submodule → sub-instance`
tree survives elaboration unflattened (Part II §7).

`generate` regions (`for`/`if`/`case`, ch. 6) are unrolled by `UnrollGenerate` (Part II
§4) into a derived instance set that never touches the authored module's own instance
list — reading a generate-bearing module back through the POM (Part IV) shows exactly
the `for`/`if`/`case` construct as written, not its expansion.

`connectrules`/`connect` (ch. 7, §10) — the automatic connect-module-selection
mechanism at a discipline-domain crossing — is parsed and its selection algorithm is
implemented (`InsertConnectModules`, Part II §4), but **cannot be exercised by any real
`.vams` source today**: `ResolveDisciplines` (the pass immediately before it) already
hard-fails on the only inputs that would trigger connect-module selection. This is a
genuine, documented gap in ch. 7 coverage, not a hidden one.

## §6 The digital subset: what's live vs. parse-only (LRM ch. 7, 8)

`always @(...)`/`initial` blocks (`BehaviorKind::Always`/`Initial`) are the one part of
the digital surface with a real, live path to a simulated device — Part IX/X's fused
digital-network JIT compiles `LoweredBody` module semantics built from these blocks.
`crates/piperine-lang/tests/vams_vertical_slice.rs::vams_digital_counter_register_
increments_on_posedge` is the live proof: an `always @(posedge clk) count <= count + 1;`
counter drives a real compiled digital device with the `SUPPORTS_ROLLBACK` capability,
asserted to increment exactly `+5` for five forced clock edges.

Everything else in ch. 7/8's surface is **parsed correctly and given an independently
tested pure-function implementation, but has no lowering path into the POM and no
codegen wiring** — instantiating any of these in a `.vams` source parses without error
but produces no simulated behavior:

- **Gate primitives** (§7.2-§7.4: `and`/`nand`/`or`/`nor`/`xor`/`xnor`, `buf`/`not`,
  `bufif0/1`/`notif0/1`) — truth tables and drive-strength resolution are implemented
  and tested (`GatePrimitive::eval`, `crates/piperine-codegen/src/kernel/digital/
  gates.rs`), confirmed live by a discrimination-sensor mutation test (flipping
  `Nand`'s truth table to plain `And` was caught by `vams_gates::and_nand_or_
  nor_xor_xnor_truth_tables`). But `piperine-lang`'s elaboration pipeline never lowers
  a `GateInstantiation` AST node into the POM, so no instance of `and g1(y, a, b);`
  ever reaches a compiled device.
- **User-defined primitives** (`primitive`/UDP, Annex A.5.3) — combinational and
  sequential state-table evaluation is implemented (`UdpTable`,
  `crates/piperine-codegen/src/kernel/digital/udp.rs`) against the same
  "pure, tested, not wired" pattern.
- **Switch-level primitives** (`nmos`/`pmos`/`cmos`, `tran*`, `pullup`/`pulldown`) —
  not even given a pure-function implementation; they are a bidirectional-resistive
  network model, structurally different from a truth table, and no work has started.
- **LRM ch. 8's stratified event-region cycle** (`Active`/`Inactive`/`Nba`/`Monitor`,
  §8.3) — `RegionQueue` (`crates/piperine-solver/src/digital/regions.rs`) proves the
  region-priority-then-FIFO ordering rule correct in isolation (confirmed by a
  discrimination-sensor mutation on `RegionEvent::cmp`'s sort key), but is not wired
  into the live scheduler, which still runs only a two-phase (register-commit, then
  combinational re-evaluation) delta cycle.

None of this is a hidden gap — every file above carries its own module-doc
`SPEC_DEVIATION` naming exactly what's missing and why closing it was out of the task's
stated scope when it was written (T48/T49/T51 respectively).

**A wider, less-documented version of the same gap, found while grounding this Part
directly in `elab_module`'s per-`ModuleItem` match** (`crates/piperine-lang/src/elab/
lower/vams.rs`, `ElabModules`'s implementation, Part II §4): the match arm handling one
`ModuleItem` at a time silently no-ops (`=> {}`) on **`Ground`** (explicit `ground net;`
reference-ground declarations, §3.6.4), **`Event`** (named `event` declarations, used
by `-> event_name` triggers), **`Specparam`**/**`Defparam`** (above), **`Function`**/
**`Task`** declared *inside* a module body (as opposed to top-level — LRM permits both;
only the top-level form has any further path through the pipeline, and even that path
dead-ends at codegen per §7), **`ContinuousAssign`** (`assign lhs = rhs;`, ch. 6's
digital continuous-assignment construct), **`Specify`** (specify blocks, ch. 14 timing
checks), and **`Gate`** (gate instantiation, consistent with the dedicated finding
above). `LoopGenerate`/`IfGenerate`/`CaseGenerate` appear in the same source-level arm
list as unhandled, but are **not actually a live gap** — `UnrollGenerate` (Part II §4)
re-scans a module's `ModuleItem`s independently through its own dedicated match
(`elab/lower/generate.rs`) and does handle all three; the code comment adjacent to the
no-op arm ("genvar/generate (T25)... not yet lowered — later tasks extend this match")
is stale relative to that later pass having shipped. The remaining eight constructs in
that arm have no such second path anywhere in the pipeline — each parses, elaborates
without error, and is then silently absent from the POM.

## §7 Functions and tasks (LRM §5.9, ch. 9)

`function`/`analog function` (`FunctionKind::Digital`/`Analog`, §5.9) and `task`
declarations parse with their full port-list/local-declaration/single-body-statement
shape (Annex A.2.6/A.2.7). LRM ch. 9's system tasks/functions (`$display`,
`$temperature`, `$bound_step`, the `$itor`/`$rtoi`/... conversion family) have their
full LRM semantics implemented as pure, tested functions
(`crates/piperine-lang/src/eval/tasks.rs`, Part V §3) — but, like the gate/UDP surface
above, **no elaboration pass lowers a `$`-task call into the POM yet**. A `$display(...)`
call inside an analog block parses and elaborates, but has nowhere to go once codegen
receives the behavior body — it is not a working diagnostic print today.

## §8 Piperine's own attribute extension: `(* device *)`/`(* port *)`

Piperine's plugin system needs two pieces of metadata the LRM has no dedicated grammar
for: which plugin implements a module as a native device, and how a port maps to that
device's terminal model. Both ride entirely on **standard** Annex A.9.1
`attribute_instance` syntax — no new production, no Piperine-specific grammar. This is
covered precisely, including the recognized attribute names, the elaboration site
(`elab/lower/vams.rs::lower_module_attrs`/`lower_port_attrs`), and the one recorded
SPEC_DEVIATION (sibling `attr_spec`s rather than a nested-call form, since Annex A's
`attr_spec` grammar has no nested-call shape), in **Appendix B §B.2** — this Part does
not restate it. The governing rule restated briefly: any attribute name on a module or
port that is neither `device`/`type`/`port`/`kind` nor a name the plugin system has
itself registered fails loud (`UnknownAttrSchema`) — an unrecognized attribute is a
compile error, never silently inert. Plugins contribute no attribute schemas of their
own; the two schemas above are seeded once by the plugin system itself
(`PluginHost::seed_schemas`), so two device plugins can never collide on them.

This is the entirety of Piperine's syntactic extension over the LRM. There is no other
extension surface, no second `(* ... *)` vocabulary, and no PHDL-era "layer-2 registry"
mechanism (MD-24's `extern` declarations, capability impls, etc.) carried forward —
Part V §0 covers exactly what replaced MD-24 for builtin resolution.

## §9 Preprocessor (LRM Annex E)

`crates/piperine-lang/src/preprocessor.rs`/`annex_e.rs` implement the LRM's textual
macro/conditional-compilation layer: `` `define``/`` `ifdef``/`` `ifndef``/`` `else``/
`` `endif``/`` `include``. `` `include`` reuses the existing `SourceMap` search-path
logic shared with the rest of the compiler's file resolution. The stdlib headers
(`disciplines.vams`, `constants.vams`, Part V §4) are ordinary source files that lean on
exactly this mechanism (`` `ifdef DISCIPLINES_VAMS``-style include guards) — there is no
special-cased header-loading path distinct from an ordinary project `` `include``.

## §10 What is explicitly not supported

- **VPI (Verilog Procedural Interface) C-ABI hosting.** ch. 11's VPI mechanism is
  modeled at the language-registration level only (`VpiTaskKind`/`VpiTaskRegistry`,
  Part V §3) — a `$name` becomes valid callable surface through Piperine's own native
  plugin ABI, never through a `dlopen`'d IEEE-VPI shared object. There is no VPI C-ABI
  host anywhere in the codebase and none is planned; this is a permanent, deliberate
  scope boundary, not a temporary gap.
- **OSDI as an in-tree dependency.** Compiled Verilog-A compact models load as OSDI
  v0.4 shared libraries through the external `piperine-osdi` plugin repo — the solver
  core (`piperine-solver`) has no OSDI/libloading dependency at all (CLAUDE.md, project
  summary). This is an architectural boundary, not a language gap.
- **`laplace_*`/`zi_*` operators — corrected from stale project documentation.**
  `ROADMAP.md`'s "P1 backlog" table and CLAUDE.md's "Known gaps" section both currently
  read as though these operators are undeclared and fail loud at elaboration. That is
  **not what the source on this branch does**: `piperine-codegen`'s
  `AnalogOpRegistry::with_builtins` (`resolve/pom/analog_ops.rs`) registers all four
  `laplace_*` (`np`/`zp`/`nd`/`zd` — the LRM defines exactly four, §4.5.11; a prior
  registry-typo bug had registered three fabricated names instead, fixed under
  LRMC-C1-12/C5-01) and all four `zi_*` LRM variants, and they resolve exactly like `ddt`/
  `idt`. See Part V §2 for the full grounding and the direct quote from the registry
  source. `.disto` on MOS2/MOS3 (a Cranelift function-count explosion on the
  second/third-derivative kernel, opt-in via `SessionBuilder::disto(true)`, default
  off) is the one operator-family gap that genuinely remains open per `ROADMAP.md`.
- **The VAMS-14 constraint-block surface.** PHDL's `constraint Name { ... }` block and
  its attribute-instance-carried VAMS equivalent were never rebuilt after MD-45 deleted
  `pom::ConstraintBlock` — see Part IV §4's callout and
  `.specs/features/verilog-ams-support/validation.md` Gap 3 for the full history
  (including that the migration's own plan explicitly flagged this as a "must add a
  task before Phase 7 closes" item that was never acted on).
- **Gate primitives, UDPs, and `$`-system-task calls as live device behavior** — §6/§7
  above; parsed and semantically implemented, not wired into a simulated device.

## §11 Diagnostics and the fail-loud contract

Every elaboration failure is a typed `ElabErrorKind` variant carrying a source span,
`miette`-renderable diagnostic code, and (for the LRM-Annex-anchored E4xxx family)
actionable help text citing the specific clause — the full catalog is Part II §6. There
is no silent fallback path anywhere in this pipeline: an unrecognized attribute schema,
a discipline that cannot be resolved, a branch contributed inconsistently, a construct
outside Annex C's analog subset, or a parameter value outside its declared range are all
fail-loud, source-anchored compile errors, never a warning, a default substitution, or a
`0.0` placeholder (CLAUDE.md's "fail loud" rule, restated here for the language layer
specifically).

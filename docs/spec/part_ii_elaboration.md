# Part II — Elaboration

Elaboration transforms a parsed `.vams` source file into a resolved `Design` (Part IV):
generate constructs unrolled into derived instances, natures/disciplines/branches
resolved, structural well-formedness checked. This Part defines the real pipeline —
`crates/piperine-lang/src/elab/` — as it exists after the `verilog-ams-support`
migration, not the PHDL-era pipeline it replaced.

Elaboration is **pure and total**: the same source plus the same staged overrides
(Part IV §3) always produce the same `Design`. A solve value never controls elaboration
structure.

## Contents

- §1 The elaboration contract
- §2 Entry point and the pass pipeline
- §3 Constant evaluation
- §4 The pass catalog, in execution order
- §5 Hierarchy flattening — the one derived artifact
- §6 The E-STD error family
- §7 POM navigability (the unbreakable rule, restated for elaboration)
- §8 What the PHDL-era pipeline is not carried forward

---

## §1 The elaboration contract

Elaboration takes a parsed `crate::ast::SourceFile` and produces a `pom::Design`. What
"resolved" means for VAMS today is narrower than PHDL's own contract was: there is no
generic-parameter substitution, no bundle-port expansion, and no monomorphization
mechanism in the current pipeline (§8) — those were PHDL value-layer features with no
VAMS Annex A production to lower. What elaboration *does* guarantee: every top-level
`nature`/`discipline` is registered before any module needs it; every `module`/
`macromodule`/`connectmodule` is lowered into a POM `Module` with its ports, params,
wires, instances, branches, and behavior bodies; every `generate` region is unrolled
into a derived instance set; a fixed-point discipline-resolution pass assigns a
discipline to every net that declared none; and the analog subset (Annex C) is enforced
on every analog behavior body. The authored `instance → submodule → sub-instances` tree
stays walkable on the POM exactly as written — no elaboration pass edits
`Design::modules` to hold anything other than what the source declared (§7).

## §2 Entry point and the pass pipeline

`SourceFile::elaborate` constructs an `Elaborator` (symbol tables, pending items) and a
`Design` under construction, then runs `elab::lower::passes::PASSES` — a fixed array of
`&dyn ElabPass`, executed strictly in order, first error aborting the whole run:

```rust
pub(crate) const PASSES: &[&dyn ElabPass] = &[
    &Register,               // index every top-level item into symbol tables
    &LowerDeclarations,      // top-level `nature`/`discipline` (T23, VAMS-01)
    &ElabModules,            // module/port/param/net/instance/behavior lowering (T22, VAMS-04)
    &ResolveDisciplines,     // LRM Annex F discipline propagation (T26)
    &InsertConnectModules,   // LRM ch. 7 connect-module selection (T52)
    &UnrollGenerate,         // LRM ch. 6 generate unrolling (T25)
    &Typecheck,              // LRM Annex C analog-subset enforcement (T27, VAMS-25)
    &FlattenHierarchy,       // derived leaf-only netlist for codegen (§5)
];
```

(`crates/piperine-lang/src/elab/lower/passes.rs` — this array, its ordering rationale,
and the trait it implements are the entire pipeline definition; there is no driver logic
hidden elsewhere.) Each pass is one struct with one job — `ElabPass::run(&self, elab:
&mut Elaborator, design: &mut Design) -> Result<(), ElabError>` — and the pipeline
*order* is this array, not something a caller has to trace through a driver function.

**Why this order, in the source's own words** (`passes.rs`'s module doc):
`UnrollGenerate` runs after `ElabModules` because generate bodies instantiate modules,
so instance lowering's target-port-order lookup needs every module already registered;
it runs before `FlattenHierarchy`, which reads the generated-instance side table when
building the derived flat form. `InsertConnectModules` runs immediately after
`ResolveDisciplines` and before `UnrollGenerate`/`FlattenHierarchy` — both write only
derived-only elaborator state, so their relative order doesn't affect correctness. Both
placements are documented decisions in the file that owns them, not incidental.

## §3 Constant evaluation

`crates/piperine-lang/src/elab/const_eval.rs` — a small, direct recursive evaluator over
`crate::ast::Expr` (`BinOp`/`UnOp`/number literals/names bound in a `ConstEnv` scope
stack). This is a deliberate departure from PHDL's own const evaluator, which delegated
to `crate::eval::Interpreter` — a general tree-walking evaluator PHDL shared with
runtime const-folding. VAMS's `Expr` has no blocks, lambdas, or statements to walk, so a
small self-contained evaluator replaced the host-trait indirection; the module's own doc
comment records this as a deliberate choice, not an oversight: "`crate::eval::interp` is
rebuilt separately (T24) for whatever richer runtime const-folding VAMS actually needs
(e.g. `analog function` bodies), not reintroduced here speculatively." Errors:
`ConstEvalError::{NotConst, DivByZero, Undefined, TypeMismatch}`.

## §4 The pass catalog, in execution order

**`Register`** — indexes every top-level `Item` (from `SourceFile::items`) into the
`Elaborator`'s symbol tables (`crate::ast::Item` variants: disciplines, natures,
modules, paramsets, connectrules, …). Pure bookkeeping; no `Design` mutation.

**`LowerDeclarations`** — lowers top-level `nature`/`discipline` declarations into
`Design::natures`/`Design::disciplines` (T23, VAMS-01). Ordered before `ElabModules`
even though nothing in module lowering strictly needs it yet, so a future pass reading
these tables during module lowering can rely on them being populated.

**`ElabModules`** — lowers every registered `module`/`macromodule`/`connectmodule` into
a POM `Module` (`elab_module`, `vams.rs`, T22, VAMS-04): ports (both declaration-list
and name-list styles, LRM §3.7), nets/wires with disciplines (§3.6.3), parameters
(§3.4), instantiation (named and ordered port connections, parameter overrides, §6.3),
and behavior-block attachment. Unlike PHDL (which had a separate top-level
`BehaviorDecl` matched by name to its owning module), VAMS behavior blocks are
structurally nested `ModuleItem`s directly inside the module body — there is no
name-matching step to get wrong.

`elab_module`'s per-item match has a real, wider no-op arm than just "generate
constructs deferred to a later pass": `ModuleItem::{Ground, Event, Specparam,
Defparam, Function, Task, ContinuousAssign, Specify, Gate}` are all `=> {}` today —
parsed and passed through elaboration successfully, but silently absent from the
resulting `Module`. `LoopGenerate`/`IfGenerate`/`CaseGenerate` sit in the same arm
list but are not actually dark — `UnrollGenerate` (below) reaches them through its own
independent scan of the same `ModuleItem`s. See Part I §6's fuller writeup of this gap
(found while grounding Part I directly in this match, not carried forward from any
prior doc or task note).

**`ResolveDisciplines`** (T26, LRM Annex F) — assigns a discipline to any port/net
declared with none, by propagating the discipline of whatever it connects to.
**Documented method and scope choice, not a silent narrowing:** the LRM names two
algorithms, "basic (the default)" and "detail" (§7.4.4); this pass implements **Basic**
(§7.4.4.1) — propagate a neighbour's known discipline across a direct connection — and
explicitly leaves Detail (which additionally reasons about per-net compatible-discipline
*sets* before collapsing them) out of scope. It resolves within one module at a time,
iterated to a fixed point across every module in the design (an unresolved port takes
its discipline from any instance port it's directly connected to whose callee-side
discipline is already known); the fuller LRM algorithm — resolution across the entire
hierarchy plus vendor `connect ... resolveto` statements (§7.4.4.3, ch. 10) — is out of
scope, tracked as follow-up in `InsertConnectModules`'s own doc (below). Failure:
`ElabErrorKind::DisciplineResolutionFailed` (E4003).

**`InsertConnectModules`** (T52, LRM ch. 7, VAMS-20) — automatic connect-module
selection at a discipline-domain crossing: a discipline pair `(A, B)` is eligible for
bridging only if a registered `connectrules` block declares a `connect` rule naming
both; given eligibility, the unique `connectmodule` whose two ports' disciplines are
exactly `{A, B}` is selected (`ElabErrorKind::ConnectModuleAmbiguous`, E4005, on zero or
multiple candidates). **Documented, load-bearing SPEC_DEVIATION, still open on this
branch:** because `ResolveDisciplines` runs one pass *earlier* and already hard-fails
the instant it sees a port connected to a net whose already-resolved discipline
mismatches (`disciplines.rs:122-131`'s `Some(d) if d != callee_disc => Err(...)` arm), a
genuine cross-discipline connection in real `.vams` source **never survives to reach
this pass**. `InsertConnectModules`'s selection algorithm is implemented and
independently unit-tested (bypassing `ResolveDisciplines` to construct a `Design`
directly, the same technique `FlattenHierarchy`'s own tests use), and is wired into
`PASSES` at the documented position — but it is provably a no-op over every design in
today's corpus, since none of them can reach it with a live mismatch. Closing this for
real needs `ResolveDisciplines` itself changed to defer a connectrules-eligible mismatch
to this pass instead of failing outright — a coupled change outside T52's stated scope,
carried forward as documented debt in both passes' own module docs.

**`UnrollGenerate`** (T25, LRM ch. 6, MD-25) — unrolls `for`/`if`/`case` generate
regions (arbitrarily nested) into **derived** instances, written only to
`Elaborator::generated_instances` — never into `Design::modules`. A generated instance's
label is mangled `{label}__{genvar value}` per loop level (outermost first); a mangled-
name collision (two generated instances, or a generated instance colliding with an
authored name) fails loud with both source spans, never a silent rename.

**`Typecheck`** (T27, LRM Annex C, VAMS-25) — enforces the analog-language subset on
every `analog` behavior body: non-blocking assignment, `@(posedge)`-style event
control, and `casex` inside an `analog` block are rejected, citing "Annex C" in the
diagnostic. Every analog behavior in the design is checked independently
(`typecheck_program_accumulating` collects every failure across every behavior), while
the pass itself still returns only the *first* to satisfy the ordinary fail-fast
`ElabPass::run` contract.

**`FlattenHierarchy`** — see §5.

## §5 Hierarchy flattening — the one derived artifact

`crates/piperine-lang/src/elab/lower/flatten.rs`. Runs last, after `Typecheck`, so it
sees the fully generate-unrolled module set. Algorithm: memoized bottom-up — for each
module, clone the authored form, recurse into every child so each child is itself
already flat, keep leaf instances as-is, and inline non-leaf children's sub-instances
through a net-rename map. The result populates `Design::flat_modules` — a
`#[serde(skip)]` side map, rebuilt on every elaboration, consumed **only** by
`piperine-codegen`. `Design::modules` (the authored hierarchy) is never mutated by this
pass — this is the POM-navigability unbreakable rule in its most concrete instance (§7).

## §6 The E-STD error family

`ElabErrorKind` (`crates/piperine-lang/src/pom/error.rs`) carries every elaboration
failure as a typed, `miette`-diagnosable variant — a span, a diagnostic code, and (for
the newer VAMS-era variants) actionable help text citing the LRM clause. The full
variant catalog, grouped by code prefix as they appear in the source:

**E1xxx — parse-failure passthrough.** `VamsParse { kind, span }` (E1005) — a
structured `ParseError` from the parser surfaced through elaboration unchanged, span
converted to a `miette::SourceSpan`. This is the *only* E1xxx variant; VAMS-05's
fail-loud requirement is satisfied by carrying the parser's own typed
`ParseErrorKind` (13 variants) through rather than re-deriving a coarser message.

**E2xxx — elaboration/lowering failures** (E2001 through E2025): `ConstEval`,
`UndefinedType` (E2002), `UndefinedModule` (E2003), `NotNetCapable` (E2004),
`ContribInDigital`/`ContribInModBody`/`ForceInModBody` (E2005-E2007), `UnknownEvent`
(E2008), `AnalogEventInDigital`/`DigitalEventInAnalog` (E2009-E2010),
`MissingConstParam` (E2011), a not-reducible-to-net-reference expression (E2012),
`WidthMismatch` (E2013), a discipline-crossing requiring an explicit converter (E2014),
`UnknownBundle`/`NoSuchBundleField` (E2015-E2016), bundle-param-default type errors
(E2017-E2018), a param/bundle-field name collision (E2019), `MultipleDrivers` (E2020),
a private-item-access violation (E2021), an attribute-schema field error (E2023),
non-exhaustive `match` (E2024), and a missing native `extern` implementation (E2025 —
vestigial: nothing in the current VAMS grammar declares `extern`, so this variant has no
live producer today). `UnknownAttrSchema` (no fixed numeric code assigned in the source)
covers an unrecognized `(* ... *)` attribute name.

**E4xxx — VAMS-era, LRM-clause-anchored elaboration failures**, all carrying
actionable `help` text citing the specific clause: `BranchKindConflict` (E4001, LRM
§5.4.2 — a branch contributed as both potential and flow outside the switch-branch
exception), `AnnexCViolation` (E4002, LRM Annex C — a construct outside the enforced
analog subset), `DisciplineResolutionFailed` (E4003, LRM Annex F), `NatureBindingInvalid`
(E4004, LRM §3.6.2.1), `ConnectModuleAmbiguous` (E4005, LRM ch. 7), and
`ParamRangeViolation` (E4006, LRM §3.4.2 — a declared `from`/`exclude` range violated,
including by the parameter's own default).

**E2999** — a catch-all `Other(String)`.

Reflection-layer errors (`ReflectError`, E3xxx) are the selector/staging-time family —
covered in Part IV §3, a distinct concern from elaboration proper.

## §7 POM navigability (the unbreakable rule, restated for elaboration)

Every pass in §4 that needs a collapsed or transformed view produces it as a **separate
side artifact**, never by overwriting `Design::modules`: `Elaborator::generated_
instances` (generate unrolling) and `Design::flat_modules` (`#[serde(skip)]`, hierarchy
flattening) are the two concrete instances of this rule in the current pipeline. A
device author querying the POM (Part IV) after elaboration reads back their own
modules, instances, and hierarchy exactly as authored — internal transforms never leak
into that view. See `.specs/features/hierarchy-flattening/design.md` for the fuller
design rationale.

## §8 What the PHDL-era pipeline is not carried forward

MD-45 (2026-08-12) deleted the PHDL-syntax-only stages outright, alongside
`piperine-lang/src/parse/` itself, rather than retargeting them at VAMS: `ValidateEvents`
(the `<+`/`<-` misuse registry), `FoldGlobals` (global `enum`/`const` constant folding),
`ElabFns` (`impl`/`fn` elaboration), `AttachBehaviors` (name-matching a top-level
`BehaviorDecl` to its module — VAMS behavior blocks are nested `ModuleItem`s and need no
such step), `ElabConstraints` (dv-core `constraint` attachment — Part IV's
`ConstraintBlock` removal is the direct consequence), `ResolveCalls` (`extern`-call
resolution — there is no `extern` in VAMS), and `Typecheck` in its PHDL form (GAPS
width/discipline typechecking over the PHDL value-net lattice — replaced by the
Annex-C-shaped `Typecheck` pass in §4, a narrower and differently-scoped check).
`Register` was already VAMS-shaped from the start of the migration and needed no
change; `FlattenHierarchy` operates on the POM rather than the surface AST and likewise
needed no change when the frontend swapped underneath it. There is no PHDL-compatibility
shim anywhere in this pipeline — a `.phdl` file is simply not a construct the parser
(now merged into `piperine-lang`, MD-47) accepts.

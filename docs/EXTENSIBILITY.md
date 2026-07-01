# Language extensibility — trait + registry pattern

## Why this doc exists

Piperine's stated goal is to be the "Golang" of mixed-signal HDLs: precise
enough to specify hardware, simple enough not to become the next
Verilog-AMS/SystemVerilog dichotomy-of-two-languages mess. The way to hit
that target long-term is **not** to keep the grammar minimal forever — it's
to keep the *core* grammar minimal while making the compiler's internal
dispatch points pluggable, so new domain concepts (layout hints, placement
constraints, new analog operators, new event kinds, new model annotations)
are additive: a new Rust struct + one `register()` call, never a new match
arm sprinkled across five files.

This is not a hypothetical. Three instances of exactly this pattern exist
in the codebase today, one long-standing and two added during the NGSPICE
headers work (2026-07-01):

| Registry | File | Dispatches |
|---|---|---|
| `EventRegistry` / `EventKind` | `elab/event.rs` | `@posedge`, `@negedge`, `@change`, `@cross`, `@above` |
| `AnalogOpRegistry` / `AnalogOp` | `lowering/analog_ops.rs` | `ddt`, `idt`, `transition`, `laplace_*`, `ac_stim`, … |
| `SyscallRegistry` / `SystemFunction` | `lowering/syscalls.rs` | `$temperature`, `$vt`, `$simparam`, `$limit`, … |

Read `elab/event.rs` first if you're implementing something new — it's the
oldest and cleanest example of the shape.

## The shape (common to all three)

```rust
pub trait FooKind: Send + Sync {
    fn lower(&self, /* whatever args this dispatch point needs */) -> Output;
}

struct SpecificFoo;
impl FooKind for SpecificFoo { fn lower(&self, ...) -> Output { ... } }

pub struct FooRegistry { table: HashMap<String, /* Box or Arc */<dyn FooKind>> }
impl FooRegistry {
    fn register(&mut self, name: &str, f: impl FooKind + 'static) { ... }
    fn with_builtins() -> Self { let mut r = ...; r.register("specific_foo", SpecificFoo); r }
    pub fn lookup(&self, name: &str) -> Option<...> { self.table.get(name) }
}
```

Two variants exist depending on whether one name maps to one impl
(`EventRegistry`, `SyscallRegistry` — `Box<dyn Trait>`, one owner) or one
impl needs to answer under several names (`AnalogOpRegistry`'s `Laplace`
struct is registered five times under `laplace_np`/`laplace_zp`/… with a
`variant` field distinguishing them — needs `Arc<dyn Trait>` so the table
can hold multiple keys pointing at one instance).

**Important limitation, stated plainly:** registration happens in Rust,
at compile time (`with_builtins()` is called once, lazily, per process —
see `LazyLock` usage in `analog_ops.rs`/`syscalls.rs`). This is **not** a
runtime plugin system — a PHDL author cannot define a new `@my_event` or
`$my_syscall` from within a `.phdl` file today. That's the correct
boundary for now: these are compiler-level concepts (they change what the
IR can express), and PHDL's job is to *use* them, not define new ones.
If/when Piperine grows a package-manager story (`docs/reflection_api.md`
already anticipates this — "first path segment is the package name"), a
third-party crate could register additional `AnalogOp`/`SystemFunction`
impls via a Rust-level plugin hook without touching this crate — that's
the extensibility payoff of doing it this way instead of a match statement.

## Applying the pattern to layout/placement annotations

This is the concrete next step toward "unifying tools for auto-placement
on silicon chips" mentioned as the longer-term goal. Sketch:

### 1. Grammar: reuse the existing `Attr` mini-syntax, don't invent a new one

`parse/ast.rs` already has `Attr { name: String, expr: Expr }`, used today
only for discipline nature attributes (`potential v : Real (unit=V,
abstol=1u);`). The same `(key=expr, key=expr)` shape is the natural
surface for layout hints on a module or instance:

```
inst m1 : nmos(d, g, s, b) (x = 12.0, y = 4.0, orientation = R90);
```

No new token, no new precedence rule — `parse_attrs` (wherever the
discipline-nature parser calls it) gets reused for instance/module
declarations. This keeps the grammar minimal, per the project's core
constraint.

### 2. A new `AnnotationKind` trait + registry, parallel to the other three

```rust
// elab/annotation.rs (new)
pub trait AnnotationKind: Send + Sync {
    fn name(&self) -> &str;
    /// Validate + evaluate the attribute's arguments against a `ConstEnv`,
    /// producing whatever payload this annotation carries (position,
    /// orientation, a routing-layer hint, …).
    fn elaborate(&self, attrs: &[Attr], env: &ConstEnv) -> Result<AnnotationValue, ElabError>;
}

struct LayoutPosition; // "layout" — x, y, orientation
struct PlacementGroup; // "place_with" — cluster hint for the placer
```

`AnnotationValue` is deliberately a small closed enum (`Position{x,y,rot}`,
`Group(String)`, …) rather than an open-ended blob — keep it typed so
downstream tools don't have to parse strings.

### 3. Where it lives in the IR

Annotations are metadata, not behavior — they must never influence
simulation semantics (a placement hint changing a DC operating point would
be a serious bug class). So they get their own field, kept separate from
everything the solver reads:

```rust
// ir.rs
pub struct IrInstance {
    pub label: String,
    pub module: String,
    pub connections: Vec<IrConnection>,
    pub params: Vec<(String, IrExpr)>,
    pub annotations: Vec<IrAnnotation>,   // new — solver never reads this
}
```

`piperine-solver`'s `Device`/`CircuitInstance` construction must
deliberately ignore this field (or it isn't a real metadata boundary — add
a test that asserts changing an annotation doesn't change the compiled
device, the same discipline as the fail-loud rule elsewhere in this repo).

### 4. Consumption

A separate tool (or a `piperine-cli` subcommand, e.g. `piperine layout
export`) walks `IrProgram` reading `IrInstance.annotations`, has nothing to
do with `piperine-solver`. This is what makes the annotation mechanism
"extensible" in the product sense: adding a new annotation kind never
touches the solver or the JIT codegen path, only `elab/annotation.rs` (new
struct + register) and whatever external tool wants to read the new
`AnnotationValue` variant.

## Recommendation

Don't build this now. Land it when there's an actual consumer (a
placement/routing tool) to validate the `AnnotationValue` shape against —
designing the enum in a vacuum is how you end up needing to redesign it
after the first real user shows up. What's worth doing *today*, cheaply,
while it's fresh:

- [ ] Confirm `Attr`'s existing parser is reusable for instance/module
      declarations (it likely needs a small grammar hook in
      `parse/parser/items.rs`'s instance-decl path — same shape as the
      discipline-nature path, not a new concept).
- [ ] Reserve `annotations: Vec<IrAnnotation>` fields on `IrInstance` (and
      maybe `IrModule`) now, defaulted to `vec![]`, so the *next* PR that
      adds the registry isn't also a breaking IR change.
- [ ] When the first real annotation lands, write it as `AnnotationKind`
      from day one — don't prototype it as a special-cased field on
      `IrInstance` and registry-ify it later, that's exactly the pattern
      this doc exists to avoid repeating.

## Cross-references

- `crates/piperine-lang/src/elab/event.rs` — the reference implementation.
- `crates/piperine-lang/src/lowering/analog_ops.rs`,
  `crates/piperine-lang/src/lowering/syscalls.rs` — the two newer
  instances (2026-07-01, part of the NGSPICE-headers work).
- `docs/GAPS.md` §I.11–§I.15 — the language changes that motivated
  building the second and third registry.
- `docs/reflection_api.md` — the existing package-name-as-first-segment
  design, relevant if this ever grows a real plugin/package story.

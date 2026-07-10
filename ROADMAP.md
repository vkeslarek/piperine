# ROADMAP.md — Open work items

Distilled from the closed-out `SPEC_BENCH_GAPS.md` / `IDE_GAPS.md` handoff drafts
(2026-07-04). Everything listed in those documents that got implemented is gone; this file
keeps only what is still open. Conventions: fail-loud until closed — an unimplemented bench
task is an elaboration error (`bench_task_implemented` allowlist in
`piperine-lang/src/eval/tasks.rs`), never a silent no-op. Closing an item updates the bench
spec §11 row (`crates/piperine-bench/docs/SPEC.md`) in the same change.

---

## Bench

### `$plot(waveform, title)` (was G1)

**Spec:** bench spec §8 table row, §11 — "emit artifacts".
**Today:** elaboration-rejected (not in `bench_task_implemented`). `$write` (CSV) is the
reference `SimTask` to copy.

Sketch:
1. Artifact format: hand-rolled SVG line chart (~100 lines, zero deps, viewable anywhere).
   Axis autoscale from `Waveform.points`, polyline, title text.
2. New `Plot` struct in `piperine-bench/src/tasks.rs` implementing `SimTask`; accepts
   `(Value::Object(Waveform | ComplexWaveform), Value::Str(title))`; downcast via
   `Object::as_any` exactly like `$noise` does for `NetRef`.
3. Output path: `<title>.svg` in the CWD (same convention as `$write`); sanitize the title
   into a filename.
4. Add `"plot"` to `bench_task_implemented`; flip the spec §11 row; e2e test in
   `piperine-bench/tests/bench.rs` asserting the file exists and starts with `<svg`.

### The uniform API (was G12) — milestone

Bench spec §8 in full: public `load()` + `Design::op/tran/ac/noise` Rust surface first
(`SimSession` renamed/made public with typed config structs), Python via `pyo3` only after
the Rust surface settles. The §8 identical-shape rule is the review gate for every signature.

### `extract` / `.attach` / `.meta` (was G13)

The extensibility spec is now written — `docs/spec/part_vi_plugins.md` (Part VI). These
land as plugin-registered `SimTask`s / staging calls once the plugin system below exists.
Do not implement ahead of it; the only prep is keeping `Attribute` surfaces public on POM
nodes (they are).

---

## Codegen / solver

- `transition`, `laplace_*`, `zi_*` analog operators — recognized in the IR, fail loud at
  codegen. Each is its own companion-model follow-up.
- **`table(x, xs, ys, mode)` operator (spec Part V §2) — not registered at all.** The
  resolved form has a `Table` symbol (`lower/symbols.rs`) but `lower/pom/analog_ops.rs`
  never registers a `"table"` operator, so a PHDL `table(...)` call doesn't even reach the
  fail-loud codegen path — it resolves as an unknown function. Register it (fail-loud until
  the interpolation companion model exists), then implement 1-D lookup + interpolation.
- **`ac_stim` in *potential* contributions — DONE (2026-07-04).** A `V(p,n) <+ … + ac_stim(mag,phase)`
  now attaches the AC drive to the force branch: `FlatForce.ac_stim`, compiled to
  `force_ac_mag`/`force_ac_phase` rows in `jit/analog.rs`, stamped onto the branch-equation
  RHS in `device/analog.rs::load_ac`. This is what makes a faithful independent **voltage
  source** (`vsrc`) drive AC analysis (previously only Norton current sources could).
  Multiple `ac_stim` per contribution is still fail-loud.
- **`$limit("pnjlim", …)` — DONE for the single-junction case (2026-07-05).** Lowered in the
  JIT with the full stateful machinery: a `vold` slot per unique `$limit` appended to the
  state bank (`jit/analog.rs` `collect_limits`/`limit_update`), the pnjlim formula in
  `jit/emit.rs::emit_pnjlim`, a `vcrit` seed at device creation (ngspice MODEINITJCT), the
  *limited* Norton linearization point (`device/analog.rs::limited_volts` — `cdeq = cd −
  gd·vlim`), and a convergence veto while limiting is active (`Device::limiting_active`, the
  ngspice `Check==1`/`noncon`). `diff.rs` treats the limiter as transparent (`d/dV =
  d(vnew)/dV`). A stiff diode (5 V → 1 kΩ → 1e-14 A) now converges to its physical operating
  point — see `spec_simulation::sim_dc_diode_pnjlim_converges`.
  **Still open:** multi-junction convergence. `bjt` (coupled B-E/B-C/substrate junctions,
  base resistance) hits NaN and `mos1` (mode/vdsat discontinuities; uses gmin not `$limit`)
  stalls. Needs per-junction limited-Norton handling for shared nodes and mode-switch
  damping. `fetlim` is stubbed to identity (no current device needs it).
- **`@initial` cannot force a branch.** `@ initial { V(p,n) <- ic; }` (the SPICE `.ic`/UIC
  seed used by `dio`/`cap`/`ind`) fails loud: "statement Force … in an analog event body".
  Event bodies only support variable assignments today; an initial-condition force needs the
  solver to accept a branch constraint for the first timepoint.
- **Large analog bodies exceeding Cranelift's function-size limit — DONE (2026-07-05).**
  The residual is one straight-line block (control flow folded to branchless `select`s), so
  the emitter now does exact common-subexpression elimination keyed by `(op-tag, child Value
  ids)` — `jit/emit.rs` `CseKey`/`cse_*`. Fully-inlined `var`/helper-`fn` bodies stop
  exploding; `dio` and `mos1` compile. Also a large speedup (shared subexpressions emit once).
- `idt` AC small-signal `1/jω` admittance not stamped (contributes 0 in AC).
- `Trace.i` over time on devices with runtime state/vars — fails loud (per-step var/state
  banks are not recorded in `TransientAnalysisResult`).

## Digital

- **Fused combinational network JIT — BUILT (2026-07-05), integration pending.** A pure-
  combinational cone compiles to one Cranelift function (`jit/digital/compile.rs::NetworkComb`)
  driven by `jit/digital/network.rs::DigitalNetwork` (one `DigitalEventModel`). Tested
  standalone (`spec_simulation::digital_network_fuses_combinational_chain`). TODO: wire into
  `circuit.rs::run_digital_at` (detect cones, build the network, fall back per-device on
  clocked/analog members); fuse clocked/register members too (comb-only today). See
  `piperine-codegen/docs/DIGITAL_JIT.md`.
- **Sequential logic cannot be clocked through `$op`.** `$op` is a *pure function* of
  (design + staged overrides): `session.rs::run_op` re-elaborates and builds a **fresh**
  circuit each call, so no digital state persists between `$op`s — a register/shift-register
  can't be stepped by staging a clock across calls (each rebuild also fires a spurious X→1
  posedge). The digital kernel and *cross-module* NBA sampling are correct — proven by
  `spec_simulation::digital_cross_module_flops_sample_simultaneously` (two flip-flops in
  separate modules; the downstream flop captures the upstream's pre-edge output). Verifying
  sequential multi-module logic through a bench needs `$tran` to record digital nets over time
  (see the `Trace` gap above). Combinational multi-module logic verifies fine through `$op` —
  see `examples/17_ripple_adder_4bit`, `18_mux4_tree`, `19_multiplier_2x2`,
  `20_comparator_4bit` (all exhaustive).

## Type system

- **Optional params `T?` + `none` — DONE (2026-07-05).** `param x : Real? = none;` is now a
  first-class optional. Syntax: a trailing `?` on any type (`Type.optional`), the `none`
  literal (`Literal::None` → `Value::Option(None)`). Read through `.is_present()` /
  `.get_or(default)` (aliases `.is_some()`/`.unwrap_or()`). In the interpreter/const layer
  these evaluate on `Value::Option`; in an analog body they lower onto the parameter-presence
  mechanism — `is_present` ≡ `$param_given(x)`, `get_or(d)` ≡ `param_given ? x : d` — so it
  works per-instance without specializing the module. Test:
  `spec_simulation::sim_dc_optional_param_get_or`.
  **Follow-up:** migrate the `piperine-spice` device models off their sentinel/`$param_given`
  encoding (`bv = 1e99`, `rbm = 0`) onto `T?` now that it exists. Optional *bundle fields*
  (`model.rbm.get_or(…)`) still need the field-receiver lowering path (today only a direct
  `param.method()` receiver folds).

## Language / interpreter gaps

**Closed 2026-07-04** (each with a gate test in `piperine-bench/tests/bench.rs`):
`impl` method dispatch everywhere (interpreter via `Host::resolve_method` on tagged
`Value::Record`s; analog/digital via `Bundle::method` IR fns with `self` flattened
per-field); bench fn → sibling bench fn calls (`Callable::BenchFn`, effectful); tuple
index `t.0`; bundle-typed fn params (flattened like module bundle params, call sites
expand the argument); the lowering's silent `Real(0.0)` fallbacks for method calls and
value-layer expressions are now loud `LowerError`s. Digital nets read directly off `$op`
results (`r.v(bit_net)` → 0/1, NaN for X/Z).

Still open:
- `for` patterns can't destructure tuples (`for (a, b) in …`); loop bodies index `case.0`.
- A bundle *literal* passed as a bundle-typed argument inside an analog body must name
  every field — the declared field defaults aren't expanded at that call site yet.
- Net/instance arrays are not addressable from a bench (`tap[2]`, `bank[0]`), and a
  bench-built circuit collapses a `wire x : T[N]` array into a single net.
- A bench top module must have at least one instance (leaf top = empty circuit);
  `.i(a, b)` needs a unique two-terminal match between the named nets.
- `Trace` (transient) does not record digital net values over time — digital readback is
  `$op`-only today.

## Language server

- True scope-aware name resolution: `symbol_index::resolve_at` is still a global first-match
  lookup; hovering `p` in module `B` can show module `A`'s port. Needs the elaborator's
  name→id maps exposed as a query.
- References/rename/highlight are word-occurrence scans gated by `resolve_at`, not
  resolver-driven use-site lists; comments/strings containing the word match.
- Project-unit elaboration: documents elaborate per-file with a project `SourceMap`;
  cross-file goto/rename and per-file diagnostic fan-out need
  `ServerState.projects: HashMap<Root, ProjectState>`.
- Protocol-level tests: drive the server over real JSON-RPC via
  `lsp_server::Connection::memory()` (init → didOpen → hover/completion round-trips);
  today's tests exercise helpers only.
- Error-accumulating elaboration (first `ElabError` stops analysis) — the editor shows one
  elaboration error at a time.
- **Attribute schema support.** `@schema_name(field = value, ...)` attributes are now
  validated and populated into the POM, but the LSP and VS Code extension don't yet:
  - Show `@schema_name` in completion (autocomplete registered schema names).
  - Validate attribute arguments in-editor (red squiggles on unknown fields, wrong
    types, missing required fields).
  - Hover on `@schema_name` → show the backing bundle's fields and types.
  - Goto-definition on `@schema_name` → jump to the `@attribute(schema = "...")`
    declaration on the bundle.
  - Show `@attribute(schema = "...")` bundles in the symbol outline.

## Spec / implementation divergences (2026-07-07 spec audit)

Cases where the formal specification (`docs/new-spec/`) describes intended behavior the
compiler does not yet enforce. The spec is the contract; these are bugs/gaps to close.

- **`white_noise` / `flicker_noise` return `0.0` placeholder.** Spec (Part V §2):
  inject a noise spectral density into the contribution RHS. Code
  (`lower/pom/analog_ops.rs:204-209`) returns `0.0` — a silent stub that violates the
  no-silent-`0.0` rule (AGENTS.md). Either lower the noise stamp or make it fail-loud.
- **Keyword reservation is parser-level, not lexical.** The lexer
  (`parse/lexer.rs:14-17`) emits every keyword as `Tok::Ident(String)`; reservation is a
  parser concern. Documented as the current design in Part I §4.2; a future lexer
  refactor could tokenize keywords for robustness.
- **E2021 `PrivateItem` is defined but never raised.** The error variant exists
  (`pom/error.rs`), but privacy is enforced by *filtering* during `use` resolution, so an
  access to a private item surfaces as E2002/E2003 ("not in scope") — a worse diagnostic
  than "item exists but is private". Wire the resolver to remember filtered-out names and
  raise E2021 when one is referenced. (Part I §16 documents the current behavior.)
- **Selector axes `driver::`, `load::`, `parent::`, `ancestor::` — `AxisNotImplemented`.**
  Spec Part IV §10 defines all ten axes; `pom/selector/eval.rs` fails loud on these four
  (structural connectivity + parent chain). `driver::`/`load::` need per-net driver/load
  tracking on the POM `Net`; `parent::`/`ancestor::` need a child→parent instance link.
- **`piperine::` stdlib exemption from `pub` filtering.** Currently the resolver
  (`resolve.rs`) skips privacy filtering for `use piperine::...` — stdlib items are
  always exported regardless of `pub`. This is because the frozen header files
  (`headers/*.phdl`) don't declare their items `pub`. Fix: add `pub` to all header
  declarations and remove the exemption so the stdlib follows the same visibility rules
  as user packages.

---

## Type system explicitness — make implicit rules visible via capabilities

Several type-system rules are currently hardcoded in the compiler (typechecker widening
table, interpreter truthiness, intrinsic capability satisfaction). Expressing them as
explicit capabilities — the same mechanism already used for `Add`, `Eq`, `Ord`, etc. —
would make the rules visible, extensible, and self-documenting in the prelude.

### Conversion / widening capabilities (`From<T>`)

**Spec (Part I §6.1):** "`Boolean` widens to `Quad` implicitly; other casts are explicit
(`real(x)`, `int(x)`, `bit(x)`)."

**Today:** the widening table is hardcoded in `typecheck.rs:518-526` — a `matches!` block
listing six allowed pairs: `(Quad ← Boolean)`, `(Boolean ← Integer)`, `(Quad ← Integer)`,
`(Natural ← Integer)`, `(Boolean ← Natural)`, `(Quad ← Natural)`. Adding a new widening
requires editing the compiler; the rule is invisible to users reading the prelude.

**Goal:** express widening as a capability in the prelude so the relationship is explicit
and extensible:

```phdl
capability From<T> { fn from(v: T) -> Self; }
impl From<Boolean> for Quad { fn from(v: Boolean) -> Self { ... } }
impl From<Integer> for Natural { fn from(v: Integer) -> Self { ... } }
```

The typechecker would check a `From` bound instead of a hardcoded table. New conversions
(e.g. a future `SInt` → `Real`) would be a prelude `impl`, not a compiler change.

### Intrinsic capability satisfaction — make it explicit

**Spec (Part I §6.6):** "Primitives satisfy the relevant [operator] capabilities
intrinsically."

**Today:** `Real`, `Natural`, `Integer`, `Boolean`, `Quad` satisfy `Add`, `Sub`, `Mul`,
`Div`, `Eq`, `Ord`, `BitAnd`, `BitOr`, `BitXor`, `Not` — but there are no `impl` blocks
for this. It's hardcoded in the operator-desugar pass. A user reading the prelude sees
`capability Add { fn add(self, o: Self) -> Self; }` but never sees *who* satisfies it.

**Goal:** add explicit `impl Add for Real`, `impl Eq for Boolean`, etc. to the prelude
(or to a generated intrinsic-impls table). This makes the capability graph complete and
discoverable, and opens the door for user-defined numeric types that satisfy the same
capabilities.

### `Iterable<T>` capability

**Spec (Part III §12):** "A `for x in <expr>` is only valid in the interpreted context."

**Today:** the interpreter (`interp.rs:399-404`) hardcodes iteration over `Value::List`
and elaboration-time `Range`. A `Map<K,V>` or a user-defined collection cannot be iterated
even though it logically could be.

**Goal:** an `Iterable<T>` capability with a `fn next(self) -> Option<T>` method (or a
`fn iter(self) -> Iterator<T>`). The `for` loop would check `Iterable` instead of
hardcoding `Value::List`.

### Literal coercion rules

**Spec (Part I §6.1):** integer literals `0`/`1` serve as `Boolean`/`Quad`/`Natural`
depending on context.

**Today:** this is implicit in the typechecker widening table (same hardcoded `matches!`
block). The spec documents it but the compiler doesn't have a named mechanism for it.

**Goal:** express via the `From<T>` capability (above) or a dedicated `FromLiteral`
capability that documents which literal types coerce to which value types. This would
replace the ad-hoc integer-literal-as-Boolean rule with an explicit `impl
FromLiteral<Integer> for Boolean` in the prelude.

### Tuple types

**Spec (Part I §6.1):** tuples are listed as a value-layer collection — `(a, b, ...)`
with `.0`/`.1`/... indexing. Tuple **values** and indexing work; tuple **type
annotations** do not.

**Progress (2026-07-09, plugin-architecture branch):** the parser now parses tuple
types (`parse_type` has a `(` branch) and `ValueType::Tuple(Vec<ValueType>)` exists
(`net_type.rs`).

**Still open:** tuple type *resolution* — `resolve_type`/the typechecker have no
`Tuple` handling, so an annotation parses but is not checked against the value. Wire
resolution + checking, then test `fn foo() -> (Real, Natural)`, `var x : (Real, String)`,
`Vec<(Real, Real)>` (the bench-spec §12.4 sweep shape).

### Function references — passing named functions as arguments

**Spec (Part I §9.2):** "A function is a value: type `fn(T, U) -> R`."

**Today:** the `fn(T) -> R` type annotation **parses and resolves** — the grammar and
`ValueType::FnPtr` handle it. But the interpreter cannot **pass a named function** as an
argument. Writing `apply_op(my_func, 5.0)` where `my_func` is a top-level `fn` fails:
the interpreter resolves identifiers to values (`Value::Int`, `Value::Real`, etc.) but a
bare function name is not a `Value::Closure` — it's a `Callable::Function` that lives in
the registry, not in the value layer. Only lambdas (`|x| x * 2.0`) can be passed today,
because they evaluate to `Value::Closure` directly.

**Progress (2026-07-09, plugin-architecture branch):** landed as `Value::FnRef(String)` —
`eval_expr` on a bare `Expr::Ident` that resolves to a registered callable produces a
`FnRef`, and call sites (`interp.rs`) dispatch a local holding a `FnRef`/`Closure`
through the callable registry. **Still open:** confirm with a gate test
(`apply_op(my_func, 5.0)` end-to-end) and typecheck `FnRef` against the declared
`fn(T) -> R` annotation.

### Type inference for `var` — less verbosity

**Spec (Part III §1):** "`var name = expr;` may omit its type, inferred at interpretation
time — only valid in the interpreted context (bench)."

**Today:** in compiled contexts (`analog`/`digital`), a `var` requires an explicit type:
`var acc : Real = 0.0;`. Omitting the type is a hard error outside bench
(`behavior.rs:60-64`). In bench, the type is accepted but **ignored** — it's decorative,
not checked. There is no actual inference: the interpreter treats every value by its
runtime shape, and the typechecker doesn't infer from initializers.

**Goal:** proper type inference for initialized `var` declarations everywhere:
- `var x = 0.1;` → `Real` (literal inference)
- `var x = some_fn();` → return type of `some_fn` (call-site inference)
- `var x = a + b;` → type of `a` (binary-op inference)
- `var x = [1, 2, 3];` → `Vec<Natural>` (literal + element inference)

This eliminates the most common verbosity in PHDL without sacrificing type safety —
the type is still known at compile time, just not written by hand. `param`, ports, and
fields still require explicit types (their defaults/initializers may be absent).

**Lambda parameter Types.** Once type inference exists, lambda parameters should be
inferrable too: `|x| x * 2.0` should infer `x : Real` from the body, instead of
requiring the user to annotate every lambda parameter (today lambda params are
untyped and the interpreter handles them dynamically). This pairs with the function-
reference work above — when a lambda is passed as a `fn(T) -> U` argument, the
expected parameter types from the signature can drive inference.

### Discipline nature access by declared name

**Spec (Part I §10.1):** "the declared nature names are also available: `Temp(th)`,
`Pwr(th)`, etc."

**Today:** NOT properly implemented. The flattener (`jit/flatten.rs:472-475`)
hardcodes `"V"` as the only potential access and treats **everything else** as
`NatureKind::Flow`:

```rust
let nature_kind = match name.as_str() {
    "V" => NatureKind::Potential,
    _ => NatureKind::Flow,
};
```

So `Temp(th)` (a potential) is compiled as if it were a flow — silently wrong. `Pwr(th)`
(a flow) works by accident. The access name is never resolved against the discipline's
declared natures (`potential temp : Real; flow pwr : Real;`).

**Goal:** resolve the access name against the discipline's natures at lowering time.
When the flattener sees `Temp(th)`, it should look up `Temp` in `Thermal`'s declared
natures, find it's a `Potential`, and use `NatureKind::Potential`. This connects to the
`extern` declarations roadmap item — the accessors `V`, `I`, `Temp`, `Pwr` should be
declared as `extern fn` with signatures tied to their discipline, not hardcoded.

---

## `extern` declarations — explicit builtin contracts

### The problem

Today the compiler has three classes of builtins that are **invisible in source**:

1. **Intrinsic type+capability satisfaction.** `Real` satisfies `Add`, `Eq`, `Ord`,
   etc. — but there are no `impl` blocks. The satisfaction is hardcoded in the operator
   desugar pass. A user (or the IDE) reading the prelude sees `capability Add { fn
   add(self, o: Self) -> Self; }` but never sees *who implements it* or *what the
   contract guarantees*.

2. **Injected prelude items.** The prelude headers (`headers/*.phdl`) declare
   disciplines, bundles, enums, capabilities, and constants. But the compiler also
   injects intrinsic knowledge that lives **nowhere in source** — the math function
   table (`math.rs:46-72`, 25 fns), the analog operator registry (`analog_ops.rs`,
   21 operators), the `$`-syscall registry (`syscalls.rs`, 13 syscalls), the event
   registry (`event.rs`, 6 events). These are all implicit — a user has no way to
   discover their signatures, argument types, or semantics from PHDL source.

3. **Net type accessors.** `V(a,b)`, `I(a,b)`, `Temp(th)`, `Pwr(th)` — these are
   access functions tied to discipline natures, but their signatures are not declared
   anywhere in PHDL. The compiler generates them implicitly from the `potential`/`flow`
   declarations.

### The `extern` keyword

Introduce `extern` as a declaration modifier that tells the resolver: "the body of this
item is provided by the compiler; don't look for a source-level definition — but the
**signature is a real, checkable contract**."

```phdl
// The prelude declares the contract; the compiler provides the body.
extern fn sqrt(x: Real) -> Real;
extern fn ddt(x: Real) -> Real;
extern fn exp(x: Real) -> Real;
extern fn temperature() -> Real;       // $temperature without the $

// Intrinsic capability impls become visible:
extern impl Add for Real { fn add(self, o: Real) -> Real; }
extern impl Eq for Boolean { fn eq(self, o: Boolean) -> Boolean; }

// Discipline accessors are declared, not magic:
discipline Electrical {
    potential v : Real;
    flow i : Real;
}
extern fn V(a: Electrical, b: Electrical) -> Real;   // potential difference
extern fn I(a: Electrical, b: Electrical) -> Real;   // branch flow
```

### Benefits

- **IDE visibility.** Hovering `sqrt` in an editor shows the signature and doc comment
  from the `extern fn` declaration — today it shows nothing because the function isn't
  in any `.phdl` file.
- **Contract checking.** The type checker validates calls against the declared
  signature even for builtins. Today, a wrong-arity call to `ddt` might not be caught
  until codegen.
- **Discoverability.** `extern` declarations in the prelude serve as a living catalog
  of what the compiler provides. No need to cross-reference ROADMAP or code comments.
- **Extensibility.** Plugins register new `extern` items the same way — their
  signatures live in source, their bodies in the plugin.

### What this does NOT change

The `extern` keyword is purely a **declaration vs. definition** marker — like C's
`extern` or Rust's `extern "C"`. The runtime behavior of builtins is unchanged; the
compiler still dispatches to the same Rust implementations. `extern` just makes the
*contract* visible and checkable.

### Work

1. **Grammar:** `Item ::= ... | "extern" ExternItem` where `ExternItem` covers
   `FnDecl`, `ImplDecl`, and optionally `ModDecl` (for OSDI device-model stubs).
   **DONE for `fn` (2026-07-09, plugin-architecture branch).**
2. **Parser:** accept `extern` before `fn`/`impl`; the body is optional (signature-only).
   **DONE for `fn`** — `item.rs` eats `extern`, `FnDecl::parse_with_extern`, AST carries
   `is_extern`. `extern impl` still open.
3. **Elaborator:** register `extern fn` signatures in the callable registry; reject
   if a source-level body is also present (extern means "body is compiler-provided").
   **Still open** — nothing in `elab/` consults `is_extern` yet.
4. **Prelude migration:** move the math table, analog operators, syscalls, and events
   into `extern` declarations in the prelude headers. The compiler's internal tables
   cross-check against the declared signatures.
5. **LSP:** `extern` items are first-class symbols — goto-definition, hover, completion
   all work on them.

### Related: discipline accessors

The access functions `V(a,b)`, `I(a,b)`, and named natures (`Temp(th)`, `Pwr(th)`)
are currently compiler magic — generated from the `potential`/`flow`/`storage`
declarations. With `extern`, these could be declared explicitly:

```phdl
discipline Electrical {
    potential v : Real;
    flow i : Real;
}
// The compiler generates these from the nature declarations above:
extern fn V(a: Electrical, b: Electrical) -> Real;
extern fn I(a: Electrical, b: Electrical) -> Real;
```

Or better: the accessors could be **methods on the discipline** via an `impl`:

```phdl
impl Electrical {
    fn v(self, other: Electrical) -> Real;   // potential difference
    fn i(self, other: Electrical) -> Real;   // branch flow
}
```

This would replace the current free-function `V(a,b)`/`I(a,b)` with method syntax
`a.v(b)` / `a.i(b)`, and make the accessor signatures visible and checkable. The
free-function forms could remain as sugar.

---

## Plugin system — implement Part VI (spec written, zero implementation)

**Spec:** `docs/spec/part_vi_plugins.md` (Part VI, complete as of 2026-07-07).
**Implementation plan:** `Plugin plan.md` (expanded 2026-07-09 — design decisions
D1–D13, per-crate integration surface, phased delivery with gates; supersedes the
step list below where they differ). **Phases 0–2 landed 2026-07-10** — steps 2, 3,
4 (native backend), 5, and 7 below are done; open: hooks/staging (8), bench tasks
(6), scripts (9), WASM/process backends, OSDI extraction (10).
**Today:** nothing exists — no plugin crate, no `[plugins]` parsing in
`piperine-project`, no lockfile plugin entries, no TOFU flow, no `PluginError`/P0xxx
catalog, no wasmtime dependency, no `@device`/`@port` handling in `CircuitCompiler`,
no CLI script dispatch. The only prep in place: attribute schemas are validated and
populated into the POM (Part I §8), and `Attribute` surfaces are public.

Suggested order (each independently shippable, spec section in parens):

1. **POM project model** — hard prerequisite, see the section below.
2. **Manifest + discovery (§4, §5).** Parse `piperine-plugin.toml` into a permissions
   struct; `[plugins]` section in `Piperine.toml`; resolve into `target/plugins/<name>/`
   via the existing git/path resolver; `Piperine.lock` plugin entries with
   `manifest_hash`/`content_hash`. Errors: P0006 `BadManifest`, P0007 `HashMismatch`.
3. **TOFU (§3.2).** CLI approval prompt keyed by content hash, persisted to the
   lockfile; `--trust <file>` / `--no-trust` CI modes. Error: P0001 `Untrusted`.
4. **Registration contract (§6).** The `Plugin` trait: `manifest()`, `register()`
   (devices, attr schemas, bench tasks, scripts), seven no-op-default hooks. Start
   with the **native** backend (plain dlopen + one entry symbol — least new
   machinery), WASM (`wasmtime` + serialized POM views) second, out-of-process
   JSON-RPC last. The device ABI is Piperine's own `AnalogDevice`/`DigitalDevice`
   traits — never OSDI or any external model ABI (Plugin plan D13).
5. **Attribute schemas from plugins (§10).** Plugin-registered schemas join the
   `@attribute(schema=...)` registry; collision → P0003 `SchemaConflict`.
6. **Bench tasks from plugins (§6).** Plugin `SimTask`s extend the
   `bench_task_implemented` allowlist at load time — this is the landing path for
   `extract`/`.attach`/`.meta` (G13 above).
7. **Device loading (§7).** `@device(plugin=…, type=…)` + `@port(name=…)` binding:
   `CircuitCompiler` detects the attribute, skips PHDL lowering, calls the plugin's
   `DeviceFactory` with the device spec (type id, attrs, port `NetRef` bindings,
   params). Solver sees a plain `Device`. Errors: P0004 `DeviceNotRegistered`.
8. **Lifecycle hooks (§8).** The seven hook points; `transform_design` mutates only
   through the staging handle (`set_param`/`add_instance`/`add_connection`), validated
   against the module table (no-netlist-magic). Alphabetical plugin order; conflicts →
   P0008 `StagingConflict`.
9. **Custom scripts (§9).** CLI dispatcher falls through to plugin-registered
   subcommands; capability-gated host context (`fs()`, `project()`, `spawn()`,
   `log()`); `piperine plugin list`. Error: P0009 `UnknownScript`.
10. **OSDI extraction (Plugin plan D13).** Move `solver/src/osdi/` out of the
    core into an `osdi-compat` plugin: its `DeviceFactory` loads a compiled OSDI
    v0.4 `.so` and adapts it behind the native `AnalogDevice` trait. The solver
    core drops the `osdi` module and its `libloading` dependency; the OSDI test
    corpus becomes the plugin's suite. Gate: every current OSDI test passes
    through the plugin path with the in-core module deleted. Also update the
    CLAUDE.md/architecture wording ("Verilog-A models load as compiled OSDI
    shared libraries" moves from core description to plugin description).

---

## POM project model — prerequisite for Part VI (Plugins)

**Spec (Part VI):** plugins discover, load, and wire through a project model —
`Piperine.toml`, `Piperine.lock`, the resolver, plugin manifests, capability
enforcement. Today this lives entirely in `piperine-project` (a separate crate)
and the POM (`piperine-lang::pom::Design`) has no notion of a "project."

**Today:** the POM is a flat `Design` — modules, disciplines, bundles, enums,
capabilities, functions, instances. There is no concept of:
- Which file/package each item came from (erased by the resolver's text inlining).
- Project metadata (name, version, dependencies, plugins).
- The dependency graph (which package depends on which).
- Source provenance for diagnostics (which file/line produced this POM node).

**Problem:** Part VI (plugins) requires the host to know the project structure
to resolve plugin sources, apply TOFU, enforce capabilities, and wire
`@device` attributes to the right plugin. Without a project model in the POM,
the plugin system has no anchor point.

**Goal:** model the project in the POM. Two options:

1. **POM gains a `Project` node** — carries name, version, dependencies, plugin
   sources, lockfile state. Each item in the `Design` carries its source
   package. This makes the POM self-describing: a tool or plugin can reflect
   over the design AND the project in one graph.

2. **Merge `piperine-project` into `piperine-lang`** — the project crate is
   small (manifest parsing, git resolver, lockfile). Folding it into the lang
   crate eliminates a crate boundary and makes the project model available
   everywhere the POM is. The resolver becomes a module, not a separate crate.

Either way, the result is: every POM node knows where it came from, and the
project structure is queryable through the same reflection surface (Part IV)
that already serves the design.

**This is a hard prerequisite for Part VI (Plugins).** The plugin system
cannot be implemented without a project model to anchor plugin discovery,
TOFU, and capability enforcement.

---

## Extension / packaging (user-owned, deliberately out of agent scope)

VS Code extension productization, marketplace packaging, grammar/registry sync tests,
release/versioning story — see `editors/vscode/`.

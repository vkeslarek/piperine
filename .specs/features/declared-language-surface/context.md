# Declared Language Surface Context

**Gathered:** 2026-07-21
**Spec:** `.specs/features/declared-language-surface/spec.md`
**Status:** Ready for design

---

## Feature Boundary

Every name a PHDL author can reference — type, net type/discipline, bundle,
method, capability, system task, runtime operator, attribute schema — must
resolve to a **textual declaration** somewhere in the project's headers/source
(or a plugin's published extern stub). Name lookup that finds no textual
declaration is a **compile error**, never a silent fallback to a Rust-native
registry. A declaration marked `extern` is the *only* case allowed to defer
its implementation to a native registry — but its **shape** (fields, params,
types, return type) must be 100% textual, so LSP go-to-definition always lands
on a real declaration. Practical driver: a VSCode plugin where ctrl+click never
dead-ends on "magic."

---

## Implementation Decisions

### Extern declaration syntax

- Minimal-surface approach: reuse the existing `fn`/`task`/`type`/`mod`
  declaration shapes, prefixed with the `extern` modifier — no new grammar
  family, just one new modifier applicable to a handful of declaration kinds.
- Concrete forms locked:
  - `extern type Real;` — a primitive value type, no body.
  - `extern fn sin(x: Real) -> Real;` — a native function (signature only, no
    body — the body is Rust, reached through the registry).
  - `extern task $temperature() -> Real;` — a system task (`$name` identifier
    form preserved).
  - `extern operator ddt(x: Real) -> Real;` — a runtime operator (`ddt`,
    `delay`, `slew`, `transition`, `idt`, `cross`, `above`, `timer`,
    `white_noise`, `flicker_noise`, `ddx`, …).
  - `extern attribute device { plugin: String, type: String }` — an attribute
    schema (`@device`, `@port`, and plugin-contributed ones), body-shaped like
    a bundle's field list since that's what an attribute schema already is.
  - `extern impl TypeName { fn method(self, ...) -> Ret; ... }` — native
    **methods** on a type, separate from the type's own declaration, mirroring
    PHDL's existing `impl [Capability for] TypeRef { fn ... }` shape (signature
    only, no body). This is what answers the user's original "Map" example:
    `extern type Map;` (or `extern bundle Map { ... }` if field-shaped)
    declares the type, `extern impl Map { fn get(self, k: K) -> V; ... }`
    declares its native methods — ctrl+click on `.get(...)` lands here.
- One `extern` keyword covers all five category-declarations (`fn`/`task`/
  `type`/`operator`/`impl`) plus `attribute` — not five unrelated keywords.

### Phasing

- **P1 — Mechanism only.** Grammar + parser for `extern fn`/`task`/`type`/
  `operator`/`attribute`; the elaborator's name-resolution rule flips to
  declared-first (textual lookup always runs first; `extern` is the only path
  that then dispatches to a native registry; anything else unresolved is a
  compile error — the "fail loud, never silently reach into a registry"
  invariant); LSP go-to-definition resolves an `extern` declaration's own
  `decl_span` (lands on the `extern fn sin(...)` line itself, not into Rust).
- **P2 — Migrate the six found magic surfaces**, one sub-phase per surface,
  each with its own regression gate (existing suite must stay green — this is
  a **rule change + relocation**, not a behavior change):
  1. Primitive value types (`Real`, `Natural`, `Integer`, `Complex`,
     `Boolean`, `Quad`, `String`) → `extern type` in a header.
  2. Math functions (`sin`…`limexp`, the `math.rs` table) → `extern fn` in a
     new `math.phdl`-style header.
  3. System tasks (`$assert`, `$display`, `$info`/`$warn`/`$error`/`$fatal`,
     `$temperature`, `$simparam`, `$abstime`, `$mfactor`, `$bound_step`, …) →
     `extern task`.
  4. Runtime operators (`ddt`, `delay`, `slew`, `transition`, `idt`, `cross`,
     `above`, `timer`, `white_noise`, `flicker_noise`, `ddx`, `$limit`) →
     `extern operator`. Currently invisible even to `piperine-lang` (pure
     string-match inside `piperine-codegen`) — this sub-phase is the largest
     and touches the parser/elaborator boundary the most.
  5. `@device`/`@port` attribute schemas (today hardcoded in
     `piperine-plugin/src/host.rs`) → `extern attribute` in a stdlib header.
  6. Plugin-contributed attribute schemas (today dynamically registered per
     loaded plugin, zero textual anchor) → plugin-published extern stub (see
     next decision).
- Each P2 sub-phase is independently gate-checked (existing suite green,
  zero warnings) before the next starts — smaller blast radius than migrating
  all six at once.

### Plugin-contributed schemas → textual anchor

- Every plugin must publish an `extern.phdl`-style stub declaring its own
  `@device`/`@port`/custom-attribute schema surface using the same `extern
  attribute` syntax as the stdlib's own `@device`/`@port`. One textual format
  for everything — the LSP never needs to learn to read a second file type
  (e.g. a TOML manifest) to resolve a plugin-declared name.
- The project imports each loaded plugin's stub the same way it imports any
  other header (mechanism TBD at Design: auto-import on plugin load vs an
  explicit `use plugin::extern;` — left to Design, not decided here).
- Existing plugins without a published stub are an out-of-scope migration
  concern for *this* feature (see Deferred Ideas) — the mechanism must exist
  and be exercised by at least the stdlib's own `@device`/`@port`/`@rfport`;
  updating every third-party/example plugin to publish a stub is follow-up
  work once the mechanism ships.

### Cast functions are not a language exception (added 2026-07-21)

- The bare-name cast syntax (`real(x)`, `int(x)`, `bit(x)`, `Boolean(x)`,
  `Quad(x)`) is a language special case today: `elab/resolve.rs:83-95`
  hardcodes those five identifiers and rewrites `Expr::Call(Ident(name),
  [arg])` into a synthetic `Expr::Cast` node with zero textual declaration —
  a 7th magic surface, found while incorporating this decision.
- User's explicit call: exceptions are bad; do it the Rust way — casts are
  **associated (static) functions on the target type**, not bare
  identifiers with compiler-special meaning.
- Verified reusable: PHDL already parses `Type::method(...)` call syntax
  (`Expr::Path` with `::`-segments, `parse/parser/expr.rs:247-253`) — the
  same qualified-path form already used for enum variants
  (`SarState::Idle`). No new grammar needed.
- Landing spot: casts become ordinary `extern impl` associated functions —
  `extern impl Real { fn from(x: Integer) -> Real; fn from(x: Boolean) ->
  Real; ... }`, a single **overloaded** `from` per target type, resolved by
  argument type. `resolve.rs`'s hardcoded cast-name rewrite is deleted
  entirely — not migrated to `extern`, **removed**, since the whole point is
  that bare identifiers stop having compiler-special meaning.
- **Overload resolution — added to scope 2026-07-21**: user pushed back on
  the initial "distinct names per source type" default (`from_int`,
  `from_bool`) — "overload deveria ser suportado... como tudo é meio que
  resolvido em tempo de elaboração, isso deveria ser possível." Verified
  during Design: every registry today is `HashMap<String, single-value>` —
  zero overload precedent anywhere — but the user's architectural reasoning
  holds: PHDL resolves argument types concretely (const_args/type_subst)
  *before* call resolution, so candidate selection needs no bidirectional
  inference, only forward type matching. **Decision: build real overload
  resolution as part of this feature** (`CallableRegistry` and the
  impl-method table become `HashMap<String, Vec<candidate>>`; resolution
  picks the structurally-matching candidate, fails loud naming all
  candidates on zero or multiple matches) — benefits any `fn`/`extern fn`/
  `extern impl` with multiple signatures, not just casts. See `design.md`'s
  "Overload resolution" component.
- Real-usage check before locking this in: `real(...)`/`bit(...)` bare-call
  syntax appears in `crates/piperine-codegen/tests/analog_jit.rs` (13×),
  `crates/piperine-codegen/tests/digital_fusion.rs` (5×, `bit(`),
  `crates/piperine-lang/tests/examples/sar_adc.phdl` (2×), and
  `crates/piperine-lang/tests/type_casts.rs` (1×, the dedicated cast test) —
  a bounded, known migration surface, not a blind change.

### Agent's Discretion

- Exact import mechanism for a plugin's extern stub (auto-import vs explicit
  `use`) — Design decides.
- Whether `extern operator` needs additional metadata beyond signature (e.g.
  which are "runtime state" operators like `delay`/`slew` vs pure ones like
  `ddt`) — Design decides based on what the elaborator/codegen boundary
  actually needs to stay coherent.
- Whether keywords/control flow (`if`, `for`, `mod`, `bundle`, operators like
  `+`/`-`) need any textual counterpart — assumed NO (see spec Assumptions):
  these are true syntax, not referenceable identifiers a user would ctrl+click
  expecting a *definition* distinct from the grammar itself.

### Declined / Undiscussed Gray Areas → Assumptions

- Whether `extern` declarations can be **overridden/shadowed** by a
  project-local re-declaration (e.g. a project wanting to redefine `sin`) —
  not discussed; default assumption written to spec.md: **no shadowing** in
  this feature (name collision with an `extern` decl is an error, same as any
  other duplicate declaration) — revisit only if a concrete need surfaces.
- Whether the fail-loud rule applies retroactively to **already-parsed
  external plugin binaries** that predate this feature (i.e. do old compiled
  plugins break) — not discussed; default: the textual stub is checked at
  *elaboration* time against the PHDL source, not against the plugin binary's
  internal shape, so old plugins keep working as long as their manifest still
  matches a published (even newly-authored) stub — written to spec.md as an
  assumption for Design to confirm.

---

## Specific References

User's own words, kept verbatim as the north star for "done":

> "É simplesmente horrível ter que ficar adivinhando que magicamente tem um
> tipo ou net type ou método ou system task que NÃO está declarado em
> qualquer lugar."

> "Se eu referenciar um Map, ctrl + click tem que me levar a um extern bundle
> Map ou qualquer coisa do tipo que DECLARE tudo e seus formatos."

Practical acceptance bar: every one of the 6 found magic surfaces, after P2,
is ctrl+click-able in VSCode and lands on a real `extern` line.

---

## Deferred Ideas

- Migrating every third-party/example plugin in the repo (beyond the stdlib's
  own `@device`/`@port`/`@rfport`) to publish an `extern.phdl` stub — separate
  follow-up once the mechanism ships (this feature proves the mechanism, not
  a full plugin-ecosystem migration).
- Any VSCode-extension-side work (syntax highlighting for `extern`, hover
  tooltips beyond what LSP already returns) — this feature is server-side
  (`piperine-lang`/`piperine-lang-server`) only; the extension itself is
  out of scope unless it already consumes standard LSP `textDocument/
  definition`, which it does per the existing `goto_def.rs` handler.

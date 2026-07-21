# Declared Language Surface Specification

## Problem Statement

Seven categories of names a PHDL author can reference today resolve through a
**Rust-native registry with zero textual declaration anywhere in the
project**: primitive value types (`Real`, `Integer`, …), math functions
(`sin`, `pow`, …), system tasks (`$temperature`, `$assert`, …), runtime
operators (`ddt`, `delay`, `slew`, `cross`, …), the stdlib's own
`@device`/`@port` attribute schemas, plugin-contributed attribute schemas,
and — worse than merely undeclared — a genuine **language exception**: the
bare-name cast forms `real(x)`/`int(x)`/`bit(x)`/`Boolean(x)`/`Quad(x)` are
hardcoded string-matched in `elab/resolve.rs:83-95` and rewritten into a
synthetic `Expr::Cast` node, meaning these five identifiers carry
compiler-special meaning no `fn` call ever could. The user's explicit
guidance: exceptions like this are exactly what's wrong — casts should be
ordinary associated (static) functions on the target type, Rust-style, with
zero special-casing in the resolver.
Runtime operators are the worst case: they are recognized by string-match
**inside `piperine-codegen`**, invisible even to `piperine-lang`'s parser and
elaborator. The consequence, verified directly in code
(`piperine-lang-server/src/handlers/goto_def.rs:19`): LSP go-to-definition
silently does nothing on any of these — `resolve_at(...)?.decl_span?` returns
`None` because there is no textual span to return. A device author staring at
`ddt(qtotal)` or `@device(plugin = "osdi", type = "nmos")` has no way to
discover what it means except reading Rust source or asking someone. This
blocks a usable VSCode plugin: ctrl+click must never dead-end.

## Goals

- [ ] Every referenceable PHDL name — type, net type/discipline, bundle,
      method, capability, system task, runtime operator, attribute schema —
      resolves to a textual declaration in the project's headers/source (or a
      loaded plugin's published extern stub).
- [ ] Name lookup that finds no textual declaration is a compile error
      (fail loud) — never a silent fallback into a Rust registry.
- [ ] A declaration marked `extern` is the only kind allowed to defer its
      *implementation* to a native registry; its full *shape* (params, types,
      return type) is 100% textual — a type's own shape (`extern type`/
      `extern bundle`) and its native methods (`extern impl`) are declared
      separately, same split as ordinary PHDL types (the "Map" case: ctrl+click
      on `.get(...)` lands in `extern impl Map { fn get(self, k: K) -> V; }`).
- [ ] LSP `textDocument/definition` resolves every one of the seven found magic
      surfaces to a real `extern`/`extern impl` declaration line (the cast
      surface via its replacement `extern impl` associated functions) — the practical,
      demoable acceptance bar.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Migrating every third-party/example plugin to publish an extern stub | This feature proves the mechanism (stdlib's own `@device`/`@port`/`@rfport` as the exercised case); full ecosystem migration is follow-up (`context.md` Deferred Ideas). |
| VSCode extension client-side work (syntax highlighting, custom hover UI) | Server-side only (`piperine-lang`, `piperine-lang-server`); the extension already consumes standard LSP `textDocument/definition`. |
| Textual declaration for keywords/control flow/operators (`if`, `for`, `mod`, `bundle`, `+`, `-`) | These are syntax, not referenceable identifiers with a distinct definition site — see Assumptions. |
| Shadowing/overriding an `extern` declaration from project code | Not requested; would need its own conflict-resolution design — see Assumptions. |
| Any change to *what* the 6 relocated magic surfaces DO (math results, operator semantics, system task behavior) | Pure relocation + a resolution-rule change; zero behavior change, mirrors the `codegen-architecture` refactor's discipline. The 7th surface (bare-name casts) is the sole intentional exception — it is removed, not relocated (see P4-AC7). |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | --------------- | --------- | ---------- |
| Extern declaration syntax | `extern fn`/`extern task`/`extern type`/`extern operator`/`extern attribute`/`extern impl` — the `extern` modifier on existing declaration shapes, signature-only (no body) | User: minimal new syntax, reuse what already parses | y (user) |
| Native methods on a type | `extern impl TypeName { fn method(self, ...) -> Ret; ... }`, mirroring PHDL's existing `impl [Capability for] TypeRef { fn ... }` shape | User: answers the original "Map" example — the type's shape (`extern type`/`extern bundle`) and its methods (`extern impl`) are declared separately, same split as ordinary PHDL types | y (user) |
| Phasing | P1 = mechanism (grammar, fail-loud lookup rule, LSP wiring) only; P2 = migrate the 7 found surfaces (6 relocated + 1 removed-as-exception), one gated sub-phase each | User: smaller blast radius per sub-phase, easier to bisect a regression | y (user) |
| Plugin schema textual anchor | Every plugin publishes an `extern.phdl`-style stub using `extern attribute`; project imports it like any header | User: one textual format everywhere, LSP never learns a second file type | y (user) |
| Import mechanism for a plugin's extern stub | Left open — Design decides (auto-import on plugin load vs explicit `use plugin::extern;`) | User: agent's discretion | y (user, deferred to Design) |
| Keywords/control-flow/operators need no textual declaration | No — they are grammar, not referenceable identifiers a user would ctrl+click expecting a definition distinct from the language spec itself | Matches the user's own examples (types, net types, methods, system tasks) — all identifier-shaped names, never keywords | y (agent default, low-risk) |
| No shadowing of `extern` declarations by project code | A name collision with an `extern` decl is an ordinary duplicate-declaration error | Undiscussed gray area; smallest-surface default, consistent with existing duplicate-name handling; revisit only on concrete need | n (declined/undiscussed — logged per context.md) |
| Fail-loud rule and pre-existing compiled plugins | The textual stub is checked at elaboration time against PHDL source, not against the plugin binary's internal shape; an old plugin keeps working as long as a stub (even newly authored) matches its manifest | Undiscussed gray area; keeps existing plugin binaries working without a rebuild | n (declined/undiscussed — logged per context.md, Design to confirm) |
| Primitive types are "everything" too | Yes — `Real`/`Integer`/`Natural`/`Complex`/`Boolean`/`Quad`/`String` get `extern type` declarations, not treated as compiler keywords | User said "TUDO" explicitly when the discussion covered this category | y (user) |
| Bare-name casts (`real(x)`, `int(x)`, `bit(x)`, `Boolean(x)`, `Quad(x)`) | Removed as a language exception, not migrated as `extern`. Replaced by an overloaded `extern impl TypeName { fn from(x: SourceType) -> TypeName; ... }` per target type, reusing PHDL's existing `Type::method(...)` path-call syntax and the overload resolution decided above — zero new grammar beyond `extern`/`extern impl` itself. | User: "exceções de linguagem são uma bosta," do it the Rust way (associated functions on types, resolved by overload like Rust's `From`) | y (user) |
| Overload resolution (same name, multiple signatures, picked by argument type) | **In scope for this feature.** `CallableRegistry` and the `extern impl` method table store one-or-more candidates per name; resolution picks the candidate whose param types structurally match the call's (already-concrete) argument types — zero matches or more than one match is a fail-loud error naming the candidates tried. Applies to any `fn`/`extern fn`/`extern impl` method, not just casts. | User: "overload deveria ser suportado... como tudo é meio que resolvido em tempo de elaboração, isso deveria ser possível" — confirmed structurally sound: PHDL resolves argument types concretely before call resolution (const_args/type_subst precede body elaboration), so candidate selection needs no bidirectional inference. Verified zero existing overload precedent anywhere (every registry today is `HashMap<String, single-value>`) — this is a genuine new capability, not a hidden existing one. | y (user) |
| Cast associated-function naming | A single overloaded `from` per target type (`Real::from(x: Integer)`, `Real::from(x: Boolean)`, …), not distinct per-source-type names | Falls out of the overload-resolution decision above — the original "distinct names" fallback is superseded | y (user) |

**Open questions:** none — all resolved or logged above.

---

## User Stories

### P1: Fail-loud, declared-first name resolution ⭐ MVP

**User Story**: As a PHDL device author, I want any name I reference to
either resolve to a textual declaration or fail the build with a clear error,
so I never encounter behavior that "magically" works with nothing to read.

**Why P1**: The foundational rule. Nothing else in this feature matters until
resolution itself stops silently falling back to a Rust registry.

**Acceptance Criteria**:

1. WHEN elaboration resolves any identifier used as a type, net type, bundle,
   method, capability, system task (`$name`), runtime operator, or attribute
   schema name THEN it SHALL look up a **textual declaration** first (in the
   AST built from PHDL source, including headers) — never query a Rust-side
   registry (math table, system-task table, attribute-schema table, codegen's
   operator string-match) before a textual declaration is found.
2. WHEN the textual declaration found is a plain (non-`extern`) declaration
   THEN elaboration SHALL proceed exactly as today (a device author's own
   `fn`/`bundle`/`capability`/`mod` — this rule changes lookup *order*, not
   normal-declaration behavior).
3. WHEN the textual declaration found is marked `extern` THEN elaboration
   SHALL dispatch to the corresponding native registry entry (math function,
   system task, runtime operator, or attribute-schema handler) using the
   `extern` declaration's own name/arity/type signature to validate the call
   site — a signature mismatch between the `extern` declaration and the call
   site is a normal type/arity error, not a registry lookup failure.
4. WHEN no textual declaration (plain or `extern`) exists for a referenced
   name THEN elaboration SHALL fail loud, naming the unresolved identifier
   and its use site — never silently reach into a Rust registry as a
   fallback, and never emit a default/no-op value.
5. WHEN an `extern` declaration exists but its registry counterpart is
   missing (a Rust-side implementation bug, not an authoring bug) THEN
   elaboration SHALL fail loud naming the `extern` declaration and the
   missing native binding — distinct from AC4's "no declaration at all"
   error, so the two failure modes are diagnosable separately.
6. WHEN a name (`fn`, `extern fn`, `extern task`, `extern operator`, or an
   `extern impl`/`impl` method) is declared more than once with different
   parameter-type signatures THEN it SHALL be accepted as an **overload
   set**, not a duplicate-declaration error — `CallableRegistry` and the
   impl-method table store every candidate for that name.
7. WHEN a call site resolves an overloaded name THEN elaboration SHALL pick
   the candidate whose parameter types structurally match the call's
   (already-concrete) argument types; WHEN zero candidates match THEN it
   SHALL fail loud naming the call site and every candidate signature tried;
   WHEN more than one candidate matches THEN it SHALL fail loud as an
   ambiguous call, naming every matching candidate — never silently pick the
   first/last-registered candidate.

**Independent Test**: A PHDL file referencing an identifier with no
declaration anywhere (plain or `extern`) fails elaboration with a named error;
a PHDL file using `sin(x)` after `sin` gains an `extern fn` declaration in a
header still elaborates and JITs identically to today; two `extern impl`
methods with the same name and different single-param types both resolve
correctly by call-site argument type, and a call whose argument type matches
neither fails loud naming both candidates.

---

### P2: `extern` declaration syntax (grammar + parser)

**User Story**: As a stdlib/header author, I want `extern fn`/`task`/`type`/
`operator`/`attribute` declaration forms, so I can give every native surface
a textual home.

**Why P2**: P1's resolution rule needs somewhere to find `extern`
declarations — the grammar must exist before the rule can be exercised beyond
toy cases.

**Acceptance Criteria**:

1. WHEN the parser encounters `extern type Name;` THEN it SHALL produce a
   type declaration with no body, distinct from a `bundle`/`discipline`
   declaration, carrying a `decl_span` covering the `extern type Name;` line.
2. WHEN the parser encounters `extern fn name(params) -> RetType;` THEN it
   SHALL produce a function declaration with a signature but no body,
   `decl_span` covering the declaration.
3. WHEN the parser encounters `extern task $name(params) -> RetType;` THEN it
   SHALL produce a system-task declaration preserving the `$`-prefixed name
   form, `decl_span` covering the declaration.
4. WHEN the parser encounters `extern operator name(params) -> RetType;`
   THEN it SHALL produce a runtime-operator declaration, `decl_span` covering
   the declaration.
5. WHEN the parser encounters `extern attribute name { field: Type, ... }`
   THEN it SHALL produce an attribute-schema declaration whose fields carry
   the same required/optional/type shape as a bundle's fields, `decl_span`
   covering the declaration.
6. WHEN the parser encounters `extern impl TypeName { fn method(self, ...)
   -> RetType; ... }` (optionally `extern impl Capability for TypeName { ...
   }`, mirroring the existing `impl [Capability for] TypeRef` grammar) THEN
   it SHALL produce a native-method-block declaration — each method
   signature-only, no body — `decl_span` covering the `extern impl` line for
   the block and each method's own `decl_span` covering its signature line,
   so ctrl+click on `.method(...)` and on the block itself both resolve.
7. WHEN any `extern` declaration (including an individual method inside
   `extern impl`) is given a body (e.g. `extern fn sin(x: Real) -> Real {
   ... }`, or a method inside `extern impl Map { fn get(...) -> V { ... } }`)
   THEN parsing SHALL fail loud — `extern` is signature-only by construction,
   never a body with a native escape hatch.

**Independent Test**: Each of the 6 `extern` forms parses into a distinct AST
node with a correct `decl_span`; `extern impl`'s individual methods each carry
their own `decl_span`; a body on any `extern` declaration or `extern impl`
method is a parse error naming the offending declaration.

---

### P3: LSP go-to-definition on every `extern`-resolved name

**User Story**: As a device author using the VSCode plugin, I want
ctrl+click on any native name (`sin`, `$temperature`, `ddt`, `@device`) to
open its `extern` declaration, so I never have to guess.

**Why P3**: The concrete, demoable proof that P1+P2 close the original UX
complaint — without this, the mechanism exists but the pain point isn't
actually fixed for the user.

**Acceptance Criteria**:

1. WHEN `textDocument/definition` is requested at a use site of any name
   resolved through an `extern` declaration THEN the LSP SHALL return a
   `Location` pointing at that `extern` declaration's `decl_span` — the same
   `goto_def.rs` code path used for ordinary declarations, no special-casing
   needed once `decl_span` is populated.
2. WHEN `textDocument/definition` is requested at a use site of a name with
   no declaration at all (the P1-AC4 error case) THEN the LSP SHALL return no
   location (today's `None` behavior) — this AC only asserts the *previously
   magic* surfaces now behave like AC1, not that undeclared names gain a
   location.

**Independent Test**: For each of the 7 migrated magic surfaces (P2 stories
below), a `textDocument/definition` request on a real use site (e.g. `sin(x)`
in a stdlib header, `ddt(qtotal)` in `diode.phdl`, `@device(plugin = ...)` in
a plugin-backed module, or a native method call resolved through `extern
impl`) returns a `Location` inside the relevant `extern` declaration.

---

### P4: Migrate the seven found magic surfaces

**User Story**: As a device author, I want every currently-magic name (types,
math, system tasks, runtime operators, `@device`/`@port`, plugin schemas,
bare-name casts) authored as `extern` declarations in headers — or, for
casts, removed as a language exception entirely — so the mechanism actually
covers the language instead of remaining unused scaffolding.

**Why P4**: P1–P3 build the capability; P4 is what makes it true that
"TUDO na linguagem" is declared, per the user's own bar.

**Acceptance Criteria**:

1. WHEN `piperine-lang`'s primitive types (`Real`, `Natural`, `Integer`,
   `Complex`, `Boolean`, `Quad`, `String`) are elaborated THEN each SHALL
   resolve via an `extern type` declaration in a stdlib header — the
   `ElabContext::new()` hardcoded registration list SHALL be replaced by
   parsing that header, not duplicated alongside it.
2. WHEN a math function from `math.rs`'s `MATH_FNS` table (`sin` … `limexp`)
   is called in PHDL THEN it SHALL resolve via an `extern fn` declaration; the
   Rust `MATH_FNS` table becomes the *implementation* backing, reached only
   after the `extern fn` declaration resolves the call.
3. WHEN a system task (`$assert`, `$display`, `$info`/`$warn`/`$error`/
   `$fatal`, `$temperature`, `$simparam`, `$abstime`, `$mfactor`,
   `$bound_step`, and every other task in `eval/tasks.rs` plus the codegen
   analog-context tasks) is called THEN it SHALL resolve via an `extern task`
   declaration.
4. WHEN a runtime operator (`ddt`, `delay`, `slew`, `transition`, `idt`,
   `cross`, `above`, `timer`, `white_noise`, `flicker_noise`, `ddx`, `$limit`)
   is used in an `analog`/`digital` body THEN it SHALL resolve via an `extern
   operator` declaration — including making it visible to `piperine-lang`'s
   elaborator for the first time (today it is codegen-only string-match).
5. WHEN `@device`/`@port` are used on a module/port THEN their schema SHALL
   come from an `extern attribute` declaration in a stdlib header, replacing
   the hardcoded `register_declared("device", …)`/`register_declared("port",
   …)` calls in `piperine-plugin/src/host.rs`.
6. WHEN a plugin contributes its own attribute schema (today via dynamic
   runtime registration) THEN the project SHALL resolve it from that
   plugin's published `extern.phdl`-style stub — at least one real plugin (a
   fixture or the stdlib's own OSDI-adjacent example) exercises this path
   end-to-end.
7. WHEN a PHDL source uses a bare-name cast (`real(x)`, `int(x)`, `bit(x)`,
   `Boolean(x)`, `Quad(x)`) THEN it SHALL be **rejected** — the
   special-cased rewrite in `elab/resolve.rs` (lines 83-95) SHALL be
   **deleted**, not migrated — replaced by a single overloaded
   `extern impl TypeName { fn from(x: SourceType) -> TypeName; ... }`
   per target type (e.g. `Real::from(x)` resolved by argument type, per
   P1-AC6/7's overload rule), reusing the existing `Type::method(...)`
   path-call syntax. Bare identifiers carry no compiler-special meaning
   after this AC — casts are ordinary declared, overloaded functions,
   indistinguishable in mechanism from any other `extern impl` method.

   **"Rejected" scope (clarified post-Verifier round 1, Validation
   Gap 2):** the bare-cast *mechanism* (the special-cased rewrite to a
   synthetic `Expr::Cast` node) is gone; this is the load-bearing
   claim. A stray `real(x)` call site today falls into the
   per-category progressive-enforcement "undeclared bare-identifier
   call left untouched" bucket (T11's documented scope) — it carries
   no `Expr::Cast` meaning, but piperine-lang does not yet reject it
   as a hard error. Codegen fails loud on its own terms when it can't
   resolve `real` to anything. Global "every undeclared bare-identifier
   call is a piperine-lang error" enforcement is a separate
   cross-category rule that wasn't part of this feature's task list
   (would flip enforcement for every bare name, not just the five
   former cast names).
8. WHEN a native method exists on a type without a textual `impl` block today
   (candidates to confirm at Design: capability impls like `Add`/`Sub`/`Eq`
   for primitive types, if the typechecker special-cases them structurally
   rather than dispatching through a declared `impl`) THEN it SHALL gain an
   `extern impl` declaration — folded into whichever of AC1–AC6 above the
   owning type belongs to (e.g. `Real`'s native impls ship alongside AC1's
   primitive-type migration).
9. WHEN the full existing test suite runs after each sub-phase of this
   migration THEN it SHALL pass unchanged (same count, zero new failures) —
   this is a relocation + resolution-rule change (AC1-6, AC8), or an
   intentional, spec'd removal-and-replacement (AC7's cast deletion), never
   an accidental behavior change to what any of these surfaces compute.

**Independent Test**: Per surface, grep-verify the old Rust-only
registration is gone (or now backs an `extern` declaration instead of being
queried directly), the corresponding header gained the declarations, and
`cargo test --workspace` stays green at the same count after that surface's
sub-phase lands. For AC7 specifically: `rg 'real\(|int\(|bit\(|Boolean\(|Quad\('`
over `headers/`, `tests/`, `examples/` finds zero PHDL bare-call-cast sites
after migration (the known pre-migration sites are `analog_jit.rs`,
`digital_fusion.rs`, `sar_adc.phdl`, `type_casts.rs` — enumerated in
`context.md`).

---

## Edge Cases

- WHEN two `extern` declarations (or an `extern` and a plain declaration) use
  the same name THEN elaboration SHALL fail loud as an ordinary duplicate
  declaration — no shadowing (Assumptions).
- WHEN an `extern operator`'s call site provides the wrong arity/types THEN
  elaboration SHALL report the same class of type/arity error as a normal
  `fn` call — `extern` does not weaken argument checking.
- WHEN a header declares `extern fn` with a body THEN parsing fails loud
  (P2-AC6) — never silently ignore the body.
- WHEN a project has no plugins loaded THEN `@device`/`@port` (stdlib-declared,
  **plugin-load-gated per actual code, not unconditional as earlier code
  comments claimed** — Verifier round 1, Gap 3) SHALL still load lazily:
  `headers/device_port.phdl` is parsed and seeded into `SchemaRegistry`
  only once `PluginHost` has at least one plugin loaded (`host.rs`'s
  `if self.is_empty() { return; }` gate, preserved by T23). A project
  that references `@device`/`@port` without any plugin loaded fails loud
  with `UnknownAttrSchema("device"/"port")`, exactly as it did
  pre-feature — no regression, no behavioral change. (The earlier
  spec-internal wording "unconditional per existing code comments" was
  inaccurate; the actual pre-existing behavior is plugin-load-gated, and
  T23 preserved it.)
- WHEN a loaded plugin does not yet publish an `extern.phdl` stub (pre-this-
  feature plugin) THEN behavior is Out of Scope for this feature (see Out of
  Scope) — SHALL fail loud naming the missing stub, not silently keep using
  the old dynamic-registration path (consistent with the "no silent registry
  fallback" rule; the plugin simply needs updating, tracked as follow-up).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| DLS-01 | P1 Fail-loud declared-first resolution | Design | ✅ Verified (round 1) |
| DLS-02 | P1 Plain-declaration behavior unchanged | Design | ✅ Verified (round 1) |
| DLS-03 | P1 `extern` dispatches to registry | Design | ✅ Verified (round 1) |
| DLS-04 | P1 No declaration = fail loud | Design | ✅ Verified (round 1) |
| DLS-05 | P1 `extern` w/ missing registry binding = distinct fail-loud | Design | ✅ Verified (round 1) |
| DLS-06 | P1 Overload set accepted for differing signatures | Design | ✅ Verified (round 1) |
| DLS-07 | P1 Overload resolved by arg type; 0/>1 match = fail loud | Design | ✅ Verified (round 1) |
| DLS-08 | P2 `extern type` grammar | Design | ✅ Verified (round 1) |
| DLS-09 | P2 `extern fn` grammar | Design | ✅ Verified (round 1) |
| DLS-10 | P2 `extern task` grammar | Design | ✅ Verified (round 1) |
| DLS-11 | P2 `extern operator` grammar | Design | ✅ Verified (round 1) |
| DLS-12 | P2 `extern attribute` grammar | Design | ✅ Verified (round 1) |
| DLS-13 | P2 `extern impl` grammar (native methods) | Design | ✅ Verified (round 1) |
| DLS-14 | P2 `extern` (+ `extern impl` method) + body = parse error | Design | ✅ Verified (round 1) |
| DLS-15 | P3 goto-def resolves `extern` decl_span | Design | ✅ Verified (round 1) |
| DLS-16 | P3 undeclared name still returns no location | Design | ✅ Verified (round 1) |
| DLS-17 | P4 primitive types migrated | Design | ✅ Verified (round 1) |
| DLS-18 | P4 math functions migrated | Design | ✅ Verified (round 1) |
| DLS-19 | P4 system tasks migrated | Design | ✅ Verified (round 1) |
| DLS-20 | P4 runtime operators migrated | Design | ✅ Verified (round 2 — Fix 1 closed the surviving mutant; `resolve_operator_call`'s positive path is now exercised) |
| DLS-21 | P4 `@device`/`@port` migrated | Design | ✅ Verified (round 1) |
| DLS-22 | P4 plugin schema stub path exercised | Design | ✅ Verified (round 1) |
| DLS-23 | P4 bare-name casts removed → overloaded `extern impl from` | Design | ✅ Verified (round 2 — Fix 2 clarified "rejected" scope; mechanism-level claim is the load-bearing one) |
| DLS-24 | P4 native type methods → `extern impl` | Design | ✅ Verified (round 1 — documented "none found" finding per task escape hatch) |
| DLS-25 | P4 zero regression per sub-phase | Design | ✅ Verified (round 1) |

**ID format:** `DLS-[NUMBER]`

**Coverage:** 25 total, all verified by the independent Verifier
(`validation.md`). Two spec-precision gaps closed in round 2 (Fix 1 —
DLS-20 surviving mutant via real `ExternOperatorDecl::param_types` wiring;
Fix 2 — DLS-23 spec wording clarified to match mechanism-level
implementation). Round-1 cosmetic gap (Fix 3 — EC4 plugin-load-gating
spec wording) closed in the same round-2 spec clarification pass.

---

## Success Criteria

- [ ] Referencing any undeclared name (plain or attempted-extern) fails
      elaboration with a named, loud error — zero silent registry fallback
      anywhere in `piperine-lang`/`piperine-codegen`/`piperine-plugin`.
- [ ] All 7 found magic surfaces (types, math, system tasks, runtime
      operators, `@device`/`@port`, plugin schemas, bare-name casts) are
      resolved: 6 have `extern` declarations in headers or a plugin stub,
      and bare-name casts are removed in favor of `extern impl` associated
      functions.
- [ ] `textDocument/definition` resolves a real `Location` for a use site of
      each of the 6 relocated surfaces plus the cast replacement, verified
      against the stdlib headers.
- [ ] `cargo test --workspace` green at the same test count throughout every
      sub-phase; zero rustc warnings.
- [ ] `docs/spec/` (the formal PHDL spec) reflects the new `extern` grammar —
      not just an internal refactor invisible to the language's own
      documentation.

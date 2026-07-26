# Hierarchy Flattening Design

**Spec**: `.specs/features/hierarchy-flattening/spec.md`
**Scope**: MVP = gap 1 (the flatten pass). Gaps 2 (const-arg-into-behavior)
and 3 (array-net expansion) are deferred and fail-loud, not built here.

---

## UNBREAKABLE RULE — POM navigability mirrors the source (LOCKED, user 2026-07-20)

> **POM navigability reflects the structure of the original code, never the
> internal structure of elaboration.**

A device author reads back their own modules, instances, and hierarchy from the
POM exactly as written; internal transforms (flattening above all) are codegen
concerns they must never have to know about. This is not a preference — it is an
invariant every pass must uphold. Concretely for this feature:

> Flattening MUST NOT mutate `Design::modules`. The authored module/instance
> hierarchy stays navigable as written; the flat netlist is a **separate,
> memoized artifact** (`Design::flat_modules`) consumed only by codegen.

The audit below shows every existing pass already honors this: they build the
POM from the immutable AST and only *add* or *validate*, never overwrite
authored structure (monomorphization *names* a concrete variant `urc__5` but
keeps the `x → urc__5 → segments` edge walkable — the hierarchy is intact).
`FlattenHierarchy` is the first transform that *could* collapse that tree, so it
is precisely the one the rule constrains. (Original in-place design was
destructive; rejected.)

## Architecture Overview

A new **elaboration pass** — `FlattenHierarchy` — produces a **fully-flat**
form of each module (instances list containing only *leaf* modules) and stores
it in a **new side map `Design::flat_modules`**. `Design::modules` — the
authored hierarchy — is left **untouched**. It runs after monomorphization and
behavior attachment, so it sees concrete already-`for`-unrolled,
already-monomorphized modules (`urc__5` with its 5 segment instances present).

For each module it inlines every non-leaf instance's contents — the child's
wires, sub-instances, and connections — into a *clone* of the parent, remapping
the child's net namespace via a **rename map** (child ports → the parent nets
the instance binds; child wires → fresh path-prefixed parent wires). The inlined
instance is removed *from the clone*; its sub-instances reappear under
path-prefixed labels (`x.seg0`, `x.seg1`, …). The clone lands in `flat_modules`.

Codegen (`CircuitCompiler`) consumes `flat_modules[root]` for the simulated
root, so it sees only leaf instances and its two-level-only guard
(`device/circuit.rs:389`) becomes **unreachable for well-formed input**. The
only codegen change is *which map it reads the root from* — a one-line
indirection (`design.flat_module(root)` instead of `design.module(root)`); the
build logic is untouched.

```
                elaboration PASSES (existing, all non-destructive)
  Register → ValidateEvents → FoldGlobals → ElabFns → ElabModules
           → AttachBehaviors → ResolveCalls → Typecheck
                                                    │
                                                    ▼  NEW (additive)
                                          FlattenHierarchy
                                                    │
                     ┌──────────────────────────────┴───────────────┐
                     ▼                                               ▼
         Design::modules (AUTHORED,                   Design::flat_modules (NEW,
         hierarchical, untouched —                    leaf-only, memoized —
         navigable by tools/hosts/LSP)                consumed only by codegen)
                                                                     │
                                                     CircuitCompiler reads root here
```

## Elaboration Non-Destructiveness Audit  (verified 2026-07-20)

The immutable **AST** (`elab.syms.*_decls`) is the true author reference: every
pass clones from it and none mutate it. The **POM `Design`** is a built product
that reflects lowering (unrolled `for`, monomorphized instances point at
`Base__args`) yet **preserves the navigable module/instance tree** — every
instance's `module` names a real module in `Design::modules`.

| Pass | Effect on POM | Destructive to authored structure? |
| ---- | ------------- | ---------------------------------- |
| `Register` | AST → symbol tables (`mem::take` of pending items) | No — builds `syms`; AST decls preserved there |
| `ValidateEvents` | read-only validation | No |
| `FoldGlobals` | clones discipline/bundle/enum/capability maps in; folds global consts | No — additive |
| `ElabFns` | builds impls/`fn`s from cloned decls | No — additive |
| `ElabModules` | builds each non-generic `Module` fresh from a cloned decl | No — AST untouched |
| `AttachBehaviors` | pushes behaviors (additive); drains `mono_cache`, `or_insert`s monomorph modules | No — additive; `urc[N]` generic stays in AST, `urc__5` *added* |
| `ResolveCalls` | resolves built-in casts in behavior-body expressions | No — refines expr nodes, removes no structure |
| `Typecheck` | read-only validation | No |
| **`FlattenHierarchy`** (this feature) | **writes `flat_modules` only; `modules` untouched** | **No — by construction (side artifact)** |

Finding: the codebase already honors non-destructive elaboration end-to-end.
This feature preserves that invariant rather than introducing the first
violation.

---

## Code Reuse Analysis

### Existing components to leverage

| Component | Where | Use |
| --------- | ----- | --- |
| `Module`/`Instance`/`Connection`/`Wire`/`Port` POM | `pom/module.rs` | The flatten operates entirely on these; no new POM types for structure. |
| `NetRef { net, index }` | `pom/net_type.rs` | The unit of remapping. Scalar `NetRef` remaps cleanly; `index.is_some()` into an array wire is the gap-3 fail-loud boundary. |
| `Instance.ports: Vec<NetRef>` (positional, binds `Module.ports` by index) | `pom/module.rs:121` | Port→parent-net binding is read straight off this positional list. |
| `ElabPass` trait + `PASSES` pipeline | `elab/lower/passes.rs` | `FlattenHierarchy` is one more `ElabPass`; insertion is a one-line array edit. |
| Monomorphization (`mono.rs`) + `mono_cache` drain (`AttachBehaviors`) | `elab/lower/` | Supplies the concrete `urc__N` modules the flatten consumes — **already done**. |
| `StructuralFor`/`StructuralIf` unroll | `elab/lower/module.rs` | Produces the sub-instances/wires the flatten inlines — **already done**. |
| Behavior-by-module-name lookup in codegen | `device/circuit.rs` | Leaf behaviors stay attached to leaf modules; inlined leaf instances reference them by `module` name — **no behavior copying needed**. |
| `is_ground` | `pom/net_type.rs:12` | Ground net names pass through remapping unchanged. |

### Integration points

- **`PASSES`** (`passes.rs:25`): append `&FlattenHierarchy` after `&Typecheck`.
  Flatten is a mechanical splice of already-type-validated pieces; running it
  last avoids re-typechecking giant flattened modules. It only *inserts* into
  `flat_modules`.
- **Codegen root lookup** (`device/circuit.rs`, `InstanceBuilder::new` root +
  `compiled`/`module`): read the simulated root from `design.flat_module(root)`
  instead of `design.module(root)` — a **one-line indirection**. Leaf children
  are looked up from `modules` as today (leaves have no sub-instances, their
  flat form equals the authored form). This is the *only* codegen change.
- **`device/circuit.rs:389`**: the `!child.instances().is_empty()` error stays
  as a **defensive invariant** (now unreachable for valid input) — a flatten
  bug surfaces here loudly instead of mis-compiling.
- **`Design::with_overrides_applied`** (`design.rs:444`): retarget from
  `modules[root]` to `flat_modules[root]` — the host addresses the *flat*
  netlist (the existing "flat, already-monomorphized" host contract,
  `design.rs:441`), and codegen consumes the flat form, so a non-structural
  param restamp must patch it. Label matching is unchanged: `i.name() == path`,
  and an inlined label `x.seg0` is a flat string (dots pass through, no
  hierarchical parse). For today's 2-level designs `flat_module(root)` equals
  the authored module, so **no behavior change** to existing sweeps/overrides;
  the retarget only matters once a mid-level module is flattened. **Confirmed
  compatible.**

---

## Algorithm — the flatten pass  [FLAT-01…03]

### Leaf test

```
is_leaf(m) := m.instances.is_empty()
```

A leaf is any module with no sub-instances — every stdlib primitive (R, C,
diode, the `@device` plugin devices) qualifies. A module with instances is
*mid-level* and must be inlined.

### Driver — memoized, bottom-up, writes only `flat_modules`

```
flatten_all(design):
    for name in design.modules.keys():
        flat := flatten_module(name, design, in_progress={})
        design.flat_modules.insert(name, flat)      # side map; design.modules NEVER written

flatten_module(name, design, in_progress):
    if name in design.flat_modules: return design.flat_modules[name]   # memoized
    if name in in_progress: FAIL "recursive module instantiation: <cycle>"   # FLAT cycle guard
    in_progress.insert(name)
    m := design.modules[name].clone()               # CLONE — authored module never mutated
    kept, inl_wires, inl_insts, inl_conns := [], [], [], []
    for inst in m.instances:
        child := flatten_module(inst.module, design, in_progress)   # child now flat
        if is_leaf(child):
            kept.push(inst)                          # leaf: keep as-is
        else:
            inline(inst, child) → inl_wires/inl_insts/inl_conns
    m.instances   := kept ++ inl_insts
    m.wires       := m.wires ++ inl_wires
    m.connections := m.connections ++ inl_conns
    in_progress.remove(name)
    return m                                          # caller stores in flat_modules
```

The parent is **cloned** before splicing; `design.modules[name]` is never
written. Bottom-up recursion means a child is fully flat before inlining, so
`inline` only ever splices leaf instances. `urc__5` (5 leaf segments) is already
flat — its flat form equals its authored form; the top module that instantiates
it inlines those 5 into `flat_modules[top]`, while `modules[top]` keeps the
single authored `x: urc__5` instance.

### `inline(inst, child)` — the net remapping  [FLAT-02]

Build rename map `ρ : child-net-name → parent NetRef`:

```
for i, port in enumerate(child.ports):
    ρ[port.name] = inst.ports[i]                       # child port → parent net the instance binds
for w in child.wires:
    fresh := "{inst.name()}.{w.name}"                  # path-prefixed, collision-free
    ρ[w.name] = NetRef::simple(fresh)
    emit parent wire `fresh` (same discipline as w)
```

Then splice, rewriting every `NetRef` through `remap`:

```
remap(nr):
    if is_ground(nr.net):        return nr
    if nr.net in ρ:
        if nr.index.is_some():   FAIL gap-3 (array-net into flat, deferred)   # FLAT array guard
        return ρ[nr.net]
    FAIL "net `{nr.net}` in `{child.name}` is neither a port nor a wire"      # dangling/typo guard

for s in child.instances:                              # all leaves (child is flat)
    emit instance { label: "{inst.name()}.{s.name()}", module: s.module,
                    params: s.params, ports: s.ports.map(remap) }
for c in child.connections:
    emit connection { lhs: remap(c.lhs), rhs: remap(c.rhs) }

drop `inst` from the parent
```

### Why the labels are collision-free  [FLAT-03]

`inst.name()` is unique among the parent's instances (elaboration already
enforces distinct instance labels). Prefixing every inlined wire and
sub-instance with `inst.name() + "."` therefore cannot collide with the
parent's own names, nor with a sibling instance `y`'s inlined names
(`x.*` ≠ `y.*`). Nesting composes: `top` inlining `x` (which inlined `seg0`)
yields `x.seg0`, and a further level yields `x.seg0.rc`. Deterministic,
path-unique, and still a flat bare label for the host override contract.

---

## Data Models

One new field on `Design`, one pass-local structure. No new module/instance
types.

```rust
struct Design {
    modules: HashMap<String, Module>,        // AUTHORED hierarchy — untouched by this feature
    flat_modules: HashMap<String, Module>,   // NEW: leaf-only flat forms, codegen-only
    // …existing fields…
}

impl Design {
    /// The flattened (leaf-only) form of a module, for codegen.
    /// Falls back to the authored module when it is already a leaf.
    pub fn flat_module(&self, name: &str) -> Option<&Module> {
        self.flat_modules.get(name).or_else(|| self.modules.get(name))
    }
}

/// child-net-name → parent NetRef, built per inlined instance (pass-local).
type RenameMap = HashMap<String, NetRef>;
```

`in_progress` (a `HashSet<String>` cycle guard) is a pass-local recursion
argument; the memo *is* `flat_modules` itself. The pass only ever inserts into
`flat_modules` — `modules` is read-cloned, never written.

**Serde note**: `flat_modules` is a rebuildable derived artifact — mark it
`#[serde(skip)]` (like `behaviors`) so POM serialization/`pom_serde` round-trips
the authored hierarchy only, and the flat form is regenerated on load. Keeps the
serialized POM the designer's reference, not the expanded netlist.

---

## Error Handling Strategy

| Condition | Handling | Req |
| --------- | -------- | --- |
| Recursive module instantiation (A instantiates A transitively) | Fail loud naming the cycle (`in_progress` hit) — never infinite-loop | FLAT-01 |
| A child `NetRef` names neither a port nor a wire | Fail loud naming net + child module (a typo or a dangling internal net) — preserves the "never silent" convention | FLAT-02 |
| Indexed `NetRef` into an array wire (`node[i]`) | Fail loud as **gap 3 deferred** — the array-net → flat-net expansion is not built in the MVP; authoring must use `StructuralFor`-generated scalar wires | FLAT-02 / spec §Three Gaps |
| A monomorphized module's `analog` body references `N` | Out of the flatten's hands — that is **gap 2** (`AttachBehaviors` unsubstituted); MVP `urc` authored pure-structural avoids it; a future fix substitutes `N` per variant via `Stmt::subst_const` | spec §Three Gaps |
| `N = 0` / negative | Already fails loud upstream at const-eval / monomorphization (spec Edge Cases) — flatten never sees an empty/invalid variant | spec Edge Cases |
| Instance port-count ≠ child port-count | Already caught (codegen `circuit.rs:395` today; ideally lifted into the pass for an earlier, clearer error) | FLAT-02 |

---

## Risks & Concerns

- **Override-path dots** — *retired*: confirmed `with_overrides_applied` does a
  flat string match, so `x.seg0` addresses fine. Design phase closed the one
  open question from the spec.
- **Compile cost for large N** — inlining is O(total flattened instances); a
  `urc[10000]` produces 10 000 leaf instances. Spec marks large-N perf
  explicitly out of scope; correctness first. If it bites, memoized flat forms
  already avoid re-flattening shared sub-modules.
- **Duplicated storage (`modules` + `flat_modules`)** — the non-destructive
  choice keeps both the authored hierarchy and its flat expansion in memory.
  This is the *intended* cost of the LOCKED non-destructive principle, not an
  accident: the designer's reference must survive. `flat_modules` is
  `#[serde(skip)]` so it never bloats the serialized POM, and it is rebuildable.
  If memory ever bites, flat forms can be computed lazily per simulated root
  instead of eagerly for every module — a drop-in change behind `flat_module`.
- **Gaps 2 & 3 are real but deferred** — the MVP is only safe for the
  pure-structural authoring route (leaf segments, scalar generated wires, no
  `N` in a mid-level analog body). Both deferred paths **fail loud**, never
  mis-flatten. This is the spec's stated MVP boundary.
- **Behaviors** — leaf behaviors are looked up by module name in codegen and
  never copied, so flattening cannot desync a behavior from its structure. The
  only behavior-related risk is gap 2, already fenced off.

---

## Tech Decisions

| Decision | Choice | Rationale |
| -------- | ------ | --------- |
| **Destructiveness** | **Non-destructive** — writes only `Design::flat_modules`; `Design::modules` untouched | UNBREAKABLE RULE (LOCKED): POM navigability mirrors the source, never elaboration internals. Non-negotiable. |
| Pipeline position | New `FlattenHierarchy` pass **after** `Typecheck` (last) | Flatten splices already-validated pieces; running last avoids re-typechecking large flattened modules and keeps per-module typecheck cheap. |
| Eager-all vs lazy-root | **Eager**: flatten every module into `flat_modules`, memoized | Elaboration is root-agnostic (host picks the root later); codegen must stay language-independent, so the pass cannot be codegen-driven. Lazy-per-root is a drop-in fallback behind `flat_module` if memory bites. Perf deferred per spec. |
| Label separator | `.` (dot) — `x.seg0` | Matches the existing flat-label host contract (string match, dots pass through); readable; already used conceptually in introspection labels. |
| Authored modules | Left fully intact in `Design::modules` | The designer's reference; the whole point of the non-destructive rule. |
| `flat_modules` serde | `#[serde(skip)]` (rebuildable) | Keeps the serialized POM the authored hierarchy, not the expanded netlist. |
| Codegen guard | **Keep** `circuit.rs:389` as an assertion | Turns any future flatten regression into a loud error instead of a silent miscompile. |
| Gaps 2/3 | Fail loud, not built | Spec MVP boundary; the pure-structural `urc` route needs neither. |

---

## Requirement Traceability

| Req | Story | Design section |
| --- | ----- | -------------- |
| FLAT-01 | Inline pass produces flat netlist | Algorithm §Driver; cycle guard |
| FLAT-02 | Net/port binding + remap | Algorithm §inline / §remap; Error Handling (dangling, array-net) |
| FLAT-03 | Collision-free labels + host contract | Algorithm §labels; Integration §`with_overrides_applied` |
| FLAT-04 | `urc` ngspice proof | Consumes FLAT-01..03; validation-phase test (`tests/ngspice_validation.rs` pattern) |
| FLAT-05 | Distinct kernels per shape | No new work — name-mangled monomorph (spec §Already Solved); regression guard only |
| FLAT-06 | Shared kernel, restamp non-structural | No new work — `with_overrides_applied`; regression guard |
| FLAT-07 | Sweep, zero recompile | No new work — existing restamp path; regression guard |

**Note**: FLAT-05..07 are regression guards, not implementation — they verify
the (already-working) monomorph/restamp behavior survives the new pass.

---

## Open Design Items (for review)

1. **Port-count check placement** — lift the arity check from codegen
   (`circuit.rs:395`) into the flatten pass for an earlier error, or leave it?
   (Leaning: add it to the pass; keep codegen's as backstop.)
2. **Eager-all vs lazy-per-root for `flat_modules`** — build a flat form for
   every module at pass time, or compute lazily on first `flat_module(root)`
   and memoize? Both keep `modules` intact (rule satisfied either way); it is
   purely a compile-time/memory trade. (Leaning: eager for simplicity now,
   lazy as the documented fallback.)
3. **Gap-2 fast-follow** — worth doing `Stmt::subst_const`-per-variant in the
   same feature (small, `subst_const` already exists), or strictly deferred?
   (Leaning: defer — no MVP consumer, keeps the flatten pass single-purpose.)

# Resolved Nets: Bundles Design

**Spec**: `.specs/features/resolved-nets-bundles/spec.md`
**Status**: Approved
**Approved**: 2026-11-21 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12" and, 2026-11-21, "pode ir tocando como previsto, parando só depois da revisão no item 13"

Companion of `.specs/features/resolved-nets/design.md`; its D2 and D20 put both features on
`feature/resolved-nets`.

**Amended**: 2026-11-22 - the explicit-unknowns model (MD-78, spec amendment A6). The section below rules over every older sentence of this file.
**Amended**: 2026-11-24 - arrays of bundles are a struct of arrays (D12). **Approved**: 2026-11-25 - user: "Essas pendencias, pode decidir tu".

---

## Amendment of 2026-11-22: the explicit-unknowns model (MD-78)

| Older text | Now | Decision (spec A6) |
| ---------- | --- | ------------------ |
| "One-driver means a discrete nature without `resolve`" (Handoff of B1; `BundleShape`'s `E0261` reads `NatureKind::Resolved`) | one-driver means a discrete nature that does not implement `Resolve`; `resolved-nets` T16 switches `E0261` to `NatureRules`' capability when it removes the field, and moves the `pom_bundles.rs` fixtures that use the library's `Wire` to a nature the test declares with `impl Resolve<bool>` | A6.1, A6.2 |
| `sda: Wire` (BN-06 AC 2, T15) | `sda: Sda` with `pub nature Sda { value: Driven }` and `impl Resolve<Driven> for Sda { Driven::strongest }`, declared by the fixture | A6.2 |
| a field nobody drives is unknown | it holds its initial value; a field type without `Default` and no initial value is `E0187`; T5 gives every fixture's `Bit` field or signal an initial value | A6.3 |
| `impl` on a bundle | an inherent `impl` (BN-05) as before; `impl Trait for` a bundle stays `E0134` | A6.1 |
| Gates: "Full" ran the buck, the twins and parity after every elaborator task | per task the `**Build**` line and the touched crates' tests; the full root suite, the integrated suites, parity and the twins only at the end of a batch; the buck only "with the buck" (D9) | the user's gate policy |

---

## Amendment of 2026-11-24: how a field becomes a carrier (D11, taken at T6)

The checker and the elaborator key every carrier and node by the `DeclId` of its port or signal, so `Ty::Bundle`
and a per-field walk through both would touch every site that names a carrier. Taken instead (the same data path,
one place): stage `pom/resolve/leaves.rs` gives every bundle port and signal one **leaf** declaration per field,
named by its dotted path (`tx.valid`), typed as the field is, with the direction the view gives it
(`pom/bundles.rs`: `BundleView`, `BundleLeaf`). A leaf is a `Port` or `Signal` of the arena that is **no child**:
`children` stays as authored, `Design::decls()` and positional argument binding never see it, and a field access
keeps resolving to the struct's own field (`pom_classify::bundle_field_resolves`). `Design::module_items` is the one
ordering of "children, each bundle followed by its leaves" the checker and the elaborator walk; `Design::bundle_field(expr)`
is the one question "which leaf is `tx.valid`". `C2` (`types/bundle.rs`, `Ty::Bundle`) is not written: the views
live in the leaf's direction. Rationale: MD-25 (the POM mirrors the source) is kept at the reflection surface, the
integrity check (`Design::check`) knows the table, and every rule of §5 applies to a field with no new code.

## Amendment of 2026-11-24: an array of bundles is a struct of arrays (D12, taken at T23)

**Approved**: 2026-11-25 - user: "Essas pendencias, pode decidir tu" (coordinator ruling of 2026-11-24, taken as an AFK default). D11 planned the leaves of an array of bundles once at the
declaration, one group per element (`rx[0].valid` .. `rx[n-1].valid`), so a length that is no written integer (`[Lane; N]`, a
const generic or a module param, `N + 1`) could not be planned and was a named `P0001`. Taken instead, for every length, the
written one included (one way, no special case): an array of bundles gets **one leaf per field, typed as the array of the
field's type with the owner's lengths** (`tx: [Lane; N]` has `tx.a: [Bit; N]`; a nested field `tx.inner.x: [Bit; N]`; an array
field `tx.bits: [[Bit; 2]; N]`). `Design::bundle_place` reads a chain as a `BundlePlace { decl, elements }`: the indexes before
the first field name the element (one per element level, `BundleLeaves::element_levels`), and `tx[k].a` is the element `k` of
the leaf `tx.a`. Every rule then is the one of arrays of carriers, reused unchanged: substitution and per-instance folding of
the length, typing of `tx[k].a` as the element (`Ty::element_at`), places and drives (`element_target.rs`,
`element_commits.rs` prepend the element indexes to the place's steps), element-granular `E0187`/`E0220` (checker and
`elab/drive_overlaps.rs`, `elab/element_coverage.rs`), reads and events (`elements.rs::element_lookup`), and connections by
window: `lanes[k]` given to a bundle port gives each port field the window `k` of the parent's leaf
(`elab/bundles.rs::leaf_window`, nodes `leaf_node_run`); a whole array binds whole. The views (`flip` on the element) are the
element's. Names a person reads stay the source's: `Design::element_naming` writes an element of a leaf whose levels are the
element levels as `lanes[1].a` in diagnostics, and the host reads `Top.lanes[1].a` as the leaf `[1]` of the net `Top.lanes.a`
(`piperine-api` `Probe::field_first`), naming the column as written. The IR keeps the arrays as arrays (`lanes.a` of count
N, nodes `d.p[k]`), so no IR change. Removed: the per-element groups, `is_array_element_leaf`, `leaves_of_each_element`,
`literal_index` and the `P0001` "an array of bundles whose length is not an integer written in the source". Rationale: the
general array machinery already folds generic lengths per instance; a per-instance leaf table in the elaborator's frame would
have been a bundle-only second path.

## What the code does today (`main` at `7df9e17c`)

| Concern | Where | Today |
| ------- | ----- | ----- |
| Parsing | `msl/parser/ty.rs`, `msl/ast.rs` | `flip T`, `[T; N]` sigtypes and `in`/`out` on struct fields parse; `TypeKind::Flip` reaches the POM |
| Bundle classification | `pom/resolve/classify.rs` (`bundle_structs`, a fixpoint), `StructData::bundle`, `PortKind::Bundle`, `StructClass::BundleLiteral` | the POM already knows which structs are bundles, which ports are bundle ports, and which struct expressions are bundle literals; `E0263` (a literal's missing, repeated or unknown field) is raised there |
| Carrier methods | `pom/resolve/members.rs` | `E0211` for a carrier method on an array of carriers; a bundle receiver is not refused |
| Checker | `types/check.rs`, `types/unsupported.rs` | a struct used as a type is `P0001` ("`struct` item used as a type"), `flip` is `P0001` ("`flip` type") |
| Elaborator | — | no bundle form |

On `composite-values` (phase B, not yet on `main`) structs become value types in the checker
(its T38, T39); a bundle is not a value (§7 rule 9), so the bundle arm must be decided before
the value arm in `port_type` and in field access. That is why phases 3 to 5 wait for it.

## Architecture Overview

```mermaid
flowchart TD
    S["struct Stream { out valid: Bit, in ready: Bit, out data: Bit<u8> }"] --> P["POM: bundle (classify.rs), checks of shape.rs: E0260 to E0262, E0264"]
    P --> C["Checker: Ty::Bundle { decl, view }; a field is a carrier with the view's direction"]
    C --> E["Elaborator: BundleFields flattens each bundle port and signal into carriers and nodes named by dotted path"]
    E --> I["IR: tx.valid, tx.ready, tx.data are ordinary carriers; no bundle form"]
    I --> H["Host: Top.s.tx.valid in --print, tables, records, metadata"]
```

- **POM** (MD-25): keeps the bundle, its ports with their type and view, and every literal as written.
- **Declaration checks** (phase 1, needs nothing): `BundleShape` in `pom/resolve/shape.rs`, beside `E0201`.
- **Checker** (phase 3): `Ty::Bundle { decl, view: View }` with `View::{Producer, Consumer, Observer}`;
  `flip` toggles `Producer`/`Consumer` and composes through nested bundles (§7 rules 3, 5); a field
  read `tx.valid` has type `Ty::Carrier` with the direction the view gives; writing an input field
  is `E0222`; a carrier method on the whole bundle is `E0211`.
- **Elaborator** (phase 4): `BundleFields::flatten(port or signal)` yields one IR carrier or node per
  leaf field, named `<bundle>.<field>` (nested: `tx.inner.x`); a connection of a bundle, carrier or
  literal, is one connection per leaf field of the same name (§7 rule 14). The one-driver rule is
  checked after placement per field (§7 rule 15).

## Code Reuse Analysis

| Component | Location | How to Use |
| --------- | -------- | ---------- |
| `bundle_structs`, `PortKind::Bundle`, `StructClass::BundleLiteral` | `pom/resolve/classify.rs` | the classification all checks read |
| Stage 8 shape checks | `pom/resolve/shape.rs` | `BundleShape` joins them |
| `E0211` site | `pom/resolve/members.rs` | the receiver kind `Recv::Bundle` joins `on_array` |
| Carrier types, drive rules | `types/carrier.rs`, `types/module.rs`, `NatureRules` (`resolved-nets` C2) | a field is a carrier; its view gives the direction |
| Carriers, nodes, placement | `elab/carriers.rs`, `elab/place.rs`, `elab/children.rs` | a leaf field is one carrier or node; a connection is per field |
| Struct types and field access (`composite-values` T38, T39) | `types/` | the bundle arm comes first |
| Handles (`generics` B) | `types/`, `elab/` | `self` on a bundle is a handle; `E0133` |
| Functions returning `Event` (`unknown-semantics-events` B5) | `elab/events.rs` | `on rx.beat(clk)` |

## Components

### C1. `BundleShape` (`pom/resolve/shape.rs`)

- `E0260` a struct mixing carrier and value fields (naming one of each); `E0261` a one-driver field without a direction, or a direction on another field; `E0262` `flip` on a non-bundle, `out` on a bundle port, a direction on a nested bundle field; `E0264` a bundle as a param type, a nature's type argument, a function result, or with an initial value.

### C2. Bundle types (`types/bundle.rs`, new)

- `Ty::Bundle { decl: DeclId, view: View }`, `View::flip()`, `BundleView::field_direction(field) -> Option<Direction>`; `E0264` for a bundle as an operand of a value operator and a literal outside a connection.

### C3. `BundleFields` (`elab/bundles.rs`, new)

- `BundleFields::flatten(decl, view) -> Vec<LeafField { path: String, nature, direction }>`; `BundleConnection::bind(parent, child)` field by field (carrier or literal).

### C4. Field drivers (`elab/place.rs`)

- After placement, for every one-driver leaf field, the instances whose view makes it `out`: more than one is `E0220` naming the field's path (§7 rule 15).

### C5. Host names

- No new type: a leaf field is a carrier or node named by its dotted path, so `--print`, tables, records and `Metadata` name it so; `pom/reflect` shows the bundle port with its type and view.

## Data Models

```rust
/// The way one module sees a bundle (§7, the use table).
pub enum View {
    /// `tx: Stream`: drives the `out` fields, reads the `in` fields.
    Producer,
    /// `rx: flip Stream`: every direction reversed.
    Consumer,
    /// `in s: Stream`: every one-driver field an input.
    Observer,
}
```

## Error Handling Strategy

| Error Scenario | Handling | User Impact |
| -------------- | -------- | ----------- |
| mixed struct; bad directions; `flip`/`out` misuse; bundle as a value at a declaration | `E0260`, `E0261`, `E0262`, `E0264` at `piperine check` | names the field or the position |
| a literal's field missing, repeated, unknown (exists) or of another nature | `E0263` | names the field |
| driving an input field | `E0222` | names the field's path |
| two producers of one one-driver field | `E0220` | names the field's path |
| a carrier method on the whole bundle | `E0211` | names the bundle and the method |
| a write through a handle | `E0133` (`generics` B) | names the field |

## Goldens, pins and tests that may move

| What | Task | Rule |
| ---- | ---- | ---- |
| `types_*` tests pinning "`struct` item used as a type" or "`flip` type" as `P0001` for a bundle | T6 | replaced by the acceptance tests |
| Book `natures#8`, `natures~52790298` | T21 | the refusal becomes a run (with `Slice<T>` once `generics` C is on `main`) |
| `GAPS.md` L25, §9 share, §11 rows | T21 | removed in the closing commit |
| `code_owners.tsv` rows `E0260`, `E0261`, `E0262`, `E0264` | T1 to T4 | deleted when raised (`resolved-nets` D16); done in B1 |
| fixtures of `types_*`, `elab_*` and `pom_bundles.rs` that declare a `Bit` field or signal with no initial value | T5 | each gains an initial value (`E0187` otherwise, `explicit-unknowns` R4); the assertion is unchanged; the commit body names each |
| `pom_bundles.rs` fixtures that use the library's `Wire` | `resolved-nets` T16 | a nature the test declares with `impl Resolve<bool>` |
| Transpile goldens, twin goldens, contracts | none | never |

## Risks & Concerns

| Concern | Location | Impact | Mitigation |
| ------- | -------- | ------ | ---------- |
| `composite-values` makes structs value types in the same checker paths | `types/carrier.rs` `port_type`, field access | a bundle read as a value | T5 re-reads the anchors; C2's arm is matched before the value arm, with a test (`a_bundle_port_is_not_a_value_input`) |
| Generic bundles need `generics`' generic value types | phase 7 | BN-06 AC 3 blocked | waits; the non-generic bundle stands alone |
| The Book's `natures#8` uses `Slice<T>`, a generic module | T21 | the row cannot run before `generics` C | T21 runs after phase 7; if `generics` C is late the row keeps only that refusal, owner `generics` |
| `resolved-nets` T16 rewrites `E0261`'s question while bundles B2 or B3 may be in flight on the same branch | `pom/resolve/shape.rs` | a conflict between the two features' tasks | tasks run sequentially on one branch; whichever runs second re-reads `shape.rs` (T5 here, T11 there) |

## Divergences found (current code vs docs/spec in this area), each a fix task

| Divergence | Spec | Fix |
| ---------- | ---- | --- |
| A carrier method on a whole bundle is not refused | §5.1 `E0211`, §7 rule 12 | T8 |
| Bundle declarations are accepted by `piperine check` with no rule of §7 | §7 rules 1, 2, 5, 6, 9, 10 | T1 to T4 |

## Decisions taken (user AFK)

User's standing words: "Eu gostaria que engatilhasse até o 12" and "pode ir tocando como
previsto, parando só depois da revisão no item 13".

| # | Open point | Default taken | Why |
| - | ---------- | ------------- | --- |
| D1 | Delivery against the dependencies | declaration checks now (POM, needs nothing); views, fields, literals and elaboration after `composite-values` A–B; functions after `generics` B and `unknown-semantics-events` B5; generic and array bundles after `generics` A and C and `composite-values` C | `resolved-nets` Q1 = B, same rule |
| D2 | Where the declaration checks run | POM stage 8 (`shape.rs`), beside `E0201` and `E0263`'s classification | `piperine check` reports them unreached; the POM already classifies bundles |
| D3 | How a bundle reaches the IR | flattened into its leaf carriers and nodes by dotted path; no IR bundle form | §7 rule 14; MD-25 keeps the bundle in the POM |
| D4 | How the host names a field | `<instance path>.<bundle>.<field>` | §7 rule 12; the host names nets by path |
| D5 | BN-04 AC 3's code | `E0211` | §5.1's table names bundles |
| D6 | BN-05 AC 4's code | `E0133` | §3.1.5 |
| D7 | `E0264` split | declaration positions in the POM (T4), operand and literal positions in the checker (T7) | each where the information exists |
| D8 | An observer view of a bundle with a resolved field | the resolved field (its nature implements `Resolve`, amendment) is drivable in every view, the observer's included (§7 rule 4: fields without a direction are the same in every view) | §5.2.2 any module that sees it may drive |
| D9 | Gates (amendment) | per task: the `**Build**` line and the tests of the crates it touched; the full root suite, the integrated suites, parity and the twins once at the end of each batch, never elsewhere (user, 2026-11-21: "Quero que aplique o dos gates mais leves apenas"; 2026-11-22: "Suite completa é só no fim dos lotes e ponto final!"); the buck only in a task whose `Done when` says "with the buck" (MD-77) | the user's gate policy |
| D10 | When B2 starts (amendment) | once `explicit-unknowns` phase A is on `main` (`composite-values` A–B already is): the fixtures need `E0187`'s rule before they are rewritten once | `explicit-unknowns` design "`resolved-nets-bundles`", T5 |
| D12 | How an array of bundles is laid out (amendment of 2026-11-24, T23) | struct of arrays: one leaf per field, the array of the field's type with the owner's lengths; `tx[k].a` is element `k` of `tx.a` (**Approved**: 2026-11-25 - user: "Essas pendencias, pode decidir tu") | one path for every length; reuses arrays of carriers |

## Tech Decisions

| Decision | Choice | Rationale |
| -------- | ------ | --------- |
| `View` as an enum on `Ty::Bundle` | one place answers a field's direction | code clarity: one purpose per type |
| No IR change | bundles vanish at elaboration | the plan, printer and solvers need nothing |

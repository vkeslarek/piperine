# Resolved Nets: Bundles Specification

**Approved**: 2026-11-21 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12" and, 2026-11-21, "pode ir tocando como previsto, parando só depois da revisão no item 13" (the amendment of 2026-11-21 below; the criteria of 2026-11-18 stand under the user's "Sobre as open questions, vou de aceito todas ok?")
**Amended**: 2026-11-22 - the explicit-unknowns model (MD-78), amendment A6 below; every rule it applies is a decision the user took on 2026-11-21, quoted there. No question is open.

Bundles (§7, L25): a `struct` whose fields are carriers, its views (`flip`, observer `in`),
bundle literals and functions on a bundle. The companion of `resolved-nets`: a bundle field of a
resolved nature has no direction, and a bundle field of a discrete or continuous nature follows
that spec's natures.

## Amendment of 2026-11-21 (planning, user AFK)

Written when design and tasks were planned (ROADMAP execution order, item 12). Each change is a
D-numbered default of `design.md` "Decisions taken (user AFK)".

| # | What changed since 2026-11-18 | Effect on this spec |
| - | ----------------------------- | ------------------- |
| A1 | Pure value semantics (G.71): data copy, hardware is reached by identity (§1.4.7). A bundle, like a carrier, is hardware; a handle is the one reference left (§3.1) | BN-05 AC 1 cites §1.4.7; no criterion changes |
| A2 | The codes the error tables already map: a carrier method on a whole bundle is `E0211` (§5.1, raised today in `pom/resolve/members.rs`); a write through a handle is `E0133` (§3.1, owned by `generics` B) | BN-04 AC 3 and BN-05 AC 4 name the code |
| A3 | The spec-code guard of `params-and-io` (`code_owners.tsv`, PI-13) assigns `E0260`, `E0261`, `E0262`, `E0264` to `resolved-nets` (construct L25) | BN-08 AC 2 is added |
| A4 | Delivery (design D1): the declaration checks are POM checks beside `E0263` and need nothing (phase 1, now); views, fields, literals and the elaboration wait for `composite-values` A–B on `main` (struct items as types); functions on a bundle wait for `generics` B and `unknown-semantics-events` B5; generic bundles for `generics` A and C; arrays of bundles for `composite-values` C | Dependencies and Size rewritten |
| A5 | Planned on the branch `feature/resolved-nets`, beside `resolved-nets` (its design D2) | none |

## Amendment of 2026-11-22 (A6: the explicit-unknowns model, MD-78)

`explicit-unknowns` (MD-78, on `main` at `20c285b8`) changed what "one-driver" and "resolved"
mean and removed the library's net natures. B1 (T1 to T4, on `main`) stands; B2 onward is
re-planned. Every row applies a rule the user decided on 2026-11-21
(`.specs/features/explicit-unknowns/spec.md`, "Decision" and "Answers").

| # | The user's decision (2026-11-21) | Effect on this spec |
| - | -------------------------------- | ------------------- |
| A6.1 | "passa a exigir que nós discretos só podem ser ligados a multiplos outputs com a trait Resolve" and "Traits como CAPACIDADES" (explicit-unknowns D17, D21) | A one-driver field is a field of a discrete nature that does not implement `Resolve`; a resolved field is one whose nature does. BN-01 AC 2 and AC 4 and BN-06 AC 2 are written on the capability; `impl Trait for` a bundle stays `E0134` (D17 narrows `E0134` to bundles, it does not lift it) |
| A6.2 | "pq tem Wire? Pq eu simplesmente não declaro a Nature ABC com um resolve específico e uso o tipo dela como normal?" (explicit-unknowns D20) | The library has no `Wire`. BN-01 AC 4 and BN-06 AC 2 name a nature the model declares with `impl Resolve<Driven>` (`Sda`) |
| A6.3 | "Se o registor inicia sem valor, caso ele seja Bit = Erro" and "tem Default, pode não ser iniciado, não tem default, tem que ser especificado" (MD-78 item 3, explicit-unknowns R4) | A one-driver field nobody drives holds its initial value, and one of a type without `Default` with none is `E0187`; the edge case is rewritten, and every fixture with a `Bit` field or signal and no initial value gains one |

## Problem Statement

Part II §7 has no implementation: coverage of bundles is 0 % (`GAPS.md` §9). A `struct` used as a
port type is "struct item used as a type", and `flip` is "flip type". So a valid/ready stream, a
differential pair or a memory cell's word line and bit line cannot be passed as one port. Each
field must be its own port, with its direction written twice, once per side. The stream slice of
gallery E.6, the DRAM array of gallery F.3 and every protocol monitor written on a bundle are
blocked.

## Goals

- [ ] A bundle is a carrier type: declared as a `struct` of carriers, used as a port, a signal and a connection, in the producer, consumer and observer views.
- [ ] The one-driver rule is checked field by field after the views are applied.
- [ ] A function on a bundle reads its fields through a handle, and may return an `Event`.
- [ ] The POM keeps every bundle as written; only the IR sees its fields as separate carriers.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Bundles with extra ports (G.4) | Open in the spec; it needs a decision of the spec's owner first |
| Generic bundles (`Stream<T>`) and generic modules over them (gallery E.6's `Slice<T>`) | `generics` (L7, L30). The criteria here use non-generic bundles; generic ones follow when `generics` lands (BN-06 AC 3) |
| Arrays of bundles and of carriers, `[flip Stream<T>; N]`, `wl[r]` (L24) | `composite-values`; BN-06 AC 4 applies once they exist |
| Handles in general: a function taking a carrier or a node (L8) | `generics`; BN-05 uses them |
| Functions that return `Event` (L53) | `unknown-semantics-events` (UE-04); BN-05 AC 3 uses them |
| Natures of the fields (a nature that implements `Resolve`, `VSignal`, a project's discrete nature) | `resolved-nets` |
| `impl Trait for` a bundle | stays `E0134` (§3.1.9 as `explicit-unknowns` narrowed it); an inherent `impl` on a bundle is BN-05 |
| Templates and `Fn(Cell)` bounds (L32) | `generics` |
| Running galleries E.6 and F.3 as gates (U4) | `conformance` |
| The interpreter | Frozen at the M1 analog subset (codegen context R1) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Delivery against the dependencies | The same answer as `resolved-nets` Q1 (B, phased): the declaration checks now; non-generic bundles of `Bit`, `Electrical` and project natures after `composite-values` A–B; handles and `Event` functions after `generics` B and `unknown-semantics-events` B5 (amendment A4) | One rule for both specs of this area | y - Q1 answered B, 2026-11-18 |
| How a bundle reaches the IR | Each field becomes the carrier it is, named by its dotted path (`tx.valid`); a connection of a bundle is a connection of each field of the same name (§7 rule 14). The bundle itself exists in the POM only | §7 rule 14 ("no semantics of its own") and the navigability rule (MD-25, MD-54) | y - AFK default (design D3) |
| How the host names a bundle field | By its dotted path under the instance: `Top.s.tx.valid` in `--print`, tables, records and metadata | §7 rule 12 reaches fields with `.`; the host already names nets by path | y - AFK default (design D4) |

**Open questions:** none - answered 2026-11-18, every recommendation accepted (see the top of this file).

**Implicit-requirement dimensions.** Input validation: BN-01 AC 3 to 6, BN-03 AC 3, BN-04 (`E0260` to `E0264`). Failure states: N/A because every failure is an elaboration error, listed under input validation. Idempotency and retry: N/A because elaboration is deterministic. Auth and rate limits: N/A because the tool is local. Concurrency and ordering: BN-02 AC 4 (the one-driver check after views). Data lifecycle: N/A because nothing is stored. Observability: BN-07 (host names). External dependencies: N/A. State transitions: N/A because a bundle has no behaviour with time (§7).

---

## User Stories

### P1: A bundle is declared and checked ⭐ MVP

**User Story**: As a model author, I want to declare `struct Stream { out valid: Bit, in ready: Bit, out data: Bit<u8> }` once, with each direction written from the producer's side, so that every module that uses it agrees on who drives what.

**Why P1**: §7's declaration rules; nothing else of L25 exists without them.

**Acceptance Criteria** (BN-01):

1. WHEN a `struct` has only fields that are carriers, arrays of carriers or bundles THEN the system SHALL accept it as a bundle.
2. WHEN a bundle field of a one-driver nature (a discrete nature that does not implement `Resolve`) has `in` or `out`, and every other field has none THEN the system SHALL accept the directions.
3. IF a `struct` mixes carrier fields and value fields THEN the system SHALL report `E0260` naming a field of each kind.
4. IF a one-driver field has no direction, or a field of any other nature (a nature that implements `Resolve`, such as `Sda`; `Electrical`) has one THEN the system SHALL report `E0261` naming the field.
5. IF `flip` is applied to something that is not a bundle, `out` is written on a bundle port, or `in` or `out` is written on a nested bundle field THEN the system SHALL report `E0262`.
6. IF a bundle is used as a value (a param type, a nature's type argument, a function result, an operand of a value operator) or given an initial value THEN the system SHALL report `E0264`.

**Independent Test**: Elaboration tests, one accepted bundle and one refusal per code.

---

### P1: Views ⭐ MVP

**User Story**: As a model author, I want the producer to write `tx: Stream`, the consumer `rx: flip Stream`, and a monitor `in s: Stream`, so that each side sees its own directions without restating them.

**Why P1**: The point of a bundle (§7 rules 3 to 7).

**Acceptance Criteria** (BN-02):

1. WHEN a module has a port `tx: Stream` THEN the system SHALL let it drive the `out` fields and read the `in` fields.
2. WHEN a module has a port `rx: flip Stream` THEN the system SHALL swap `in` and `out` on every one-driver field, nested bundles included, and leave fields without a direction as they are.
3. WHEN a module has a port `in s: Stream` THEN the system SHALL treat every one-driver field as an input.
4. The system SHALL check the one-driver rule field by field after views are applied: IF two modules see one one-driver field as `out` THEN the system SHALL report `E0220` naming the field's path.
5. IF a module drives a field its view makes an input THEN the system SHALL report `E0222`.
6. WHEN a module declares `signal a: Stream;` and connects it to a producer and a consumer THEN the system SHALL declare every carrier of the bundle, and the declaring module SHALL drive no field it does not write (§7 rule 7).

**Independent Test**: A compiled run of a producer, a consumer and an observer joined by one `Stream` signal, with a valid/ready handshake over three beats.

---

### P1: Connections and literals ⭐ MVP

**User Story**: As a model author, I want to pass a bundle to a port, or build one in place from separate carriers, so that a cell's word line and bit line can come from different buses.

**Why P1**: §7 rules 8 and 14; gallery F.3 builds its cells with literals.

**Acceptance Criteria** (BN-03):

1. WHEN a bundle carrier is passed to a bundle port THEN the system SHALL connect each field to the field of the same name.
2. WHEN a bundle literal `Cell { wl: w, bl: b }` is passed to a port THEN the system SHALL connect each named field to the given carrier.
3. IF a bundle literal misses a field, repeats one, names an unknown field, or gives a carrier of another nature THEN the system SHALL report `E0263` naming the field.
4. IF a bundle literal is written anywhere but a connection THEN the system SHALL report `E0264`.
5. WHEN a bundle has a field of a continuous nature (`struct Diff { p: Electrical, n: Electrical }`) THEN the system SHALL connect its nodes like ports of that nature, and probes and contributions on `d.p` and `d.n` SHALL work as on plain nodes.

**Independent Test**: A compiled op of a differential amplifier over a `Diff` port, and a compiled run of a `Cell` connected by literal.

---

### P1: Fields are carriers ⭐ MVP

**User Story**: As a model author, I want `tx.valid` to be a carrier with every carrier rule, so that I commit, define, probe and watch a field as I would a port.

**Why P1**: §7 rule 12.

**Acceptance Criteria** (BN-04):

1. WHEN a module commits to, or defines, a field its view lets it drive (`tx.valid <- true;`, `tx.data = buf;`) THEN the system SHALL apply the rules of §5 to that field.
2. WHEN a carrier method is called on a field (`tx.valid.changed()`, `rx.ready.high()`) THEN the system SHALL apply it to that field.
3. IF a carrier method is called on a whole bundle THEN the system SHALL report `E0211` naming the bundle and the method (§5.1).

**Independent Test**: A compiled run with a reaction on `rx.valid.changed()` and a definition of `tx.data`.

---

### P2: Functions on a bundle

**User Story**: As a model author, I want `impl Stream { fn beat(self, clk: Bit) -> Event }` and `impl Diff { fn vd(self) -> f64 }`, so that a protocol's beat and a pair's differential voltage are named once.

**Why P2**: §7 rule 13; it waits for handles (`generics`, L8) and `Event` functions (`unknown-semantics-events`, UE-04).

**Acceptance Criteria** (BN-05):

1. WHEN a function on a bundle takes `self` THEN the system SHALL bind `self` as a handle and SHALL make every field read-only through it, whatever the view (§7 rule 13, §3.1.1).
2. WHEN `d.vd()` is called in `analog` for `impl Diff { fn vd(self) -> f64 { v(self.p, self.n) } }` THEN the system SHALL give `v(d.p, d.n)`.
3. WHEN `on rx.beat(clk)` heads a reaction for `fn beat(self, clk: Bit) -> Event { clk.rise() && self.valid && self.ready }` THEN the reaction SHALL run exactly at the rising edges of `clk` where `valid` and `ready` are both true.
4. IF a function commits to or defines a field through a handle THEN the system SHALL report `E0133` naming the field (§3.1.5).

**Independent Test**: A compiled run of a handshake with `beat`, and an op with `vd`.

---

### P2: Nesting, generics and arrays

**User Story**: As a model author, I want bundles inside bundles, generic bundles and arrays of bundles, as §7 rule 11 allows.

**Why P2**: Each needs another feature; the plain bundle stands without them.

**Acceptance Criteria** (BN-06):

1. WHEN a bundle has a field whose type is a bundle, written plain or with `flip` THEN the system SHALL apply the views to it recursively (§7 rule 5).
2. WHEN a bundle has a field of a nature that implements `Resolve` (`sda: Sda`, with `impl Resolve<Driven> for Sda`) THEN the system SHALL let every module that sees the bundle drive that field, the observer included, and SHALL give the field `resolve` of its drives (§5.2.2, §5.2.10).
3. WHERE generic bundles exist (`generics`) the system SHALL accept `struct Stream<T: Repr> { .. }` and instantiate it per type argument.
4. WHERE arrays of carriers exist (`composite-values`, L24) the system SHALL accept an array of bundles (`[flip Stream<T>; N]`) and connect each element field by field.

**Independent Test**: A compiled run of a nested bundle with a flipped inner bundle; the generic and array cases when their features land.

---

### P2: The host and the POM see the bundle as written

**User Story**: As a user of the host and of the POM, I want bundle fields named by their path, and the bundle itself kept in the object model, so that what I read matches what I wrote.

**Why P2**: The navigability rule and the host's naming; the bundle works without them, but nobody can observe it.

**Acceptance Criteria** (BN-07):

1. WHEN a bundle field is printed (`--print Top.p.tx.valid`), recorded or listed in metadata THEN the system SHALL name it by its dotted path under the instance.
2. WHEN the POM of a module with a bundle port is walked THEN the system SHALL show the port with its bundle type and view, not one port per field.

**Independent Test**: A `piperine-api` test printing a field by path, and a POM test over a module with a `flip` port.

---

### P3: The documents say what is true

**User Story**: As a reader of `GAPS.md` and the Book, I want them to say that bundles run.

**Why P3**: No behaviour depends on it.

**Acceptance Criteria** (BN-08):

1. WHEN this feature merges THEN `GAPS.md` SHALL no longer list L25, and §9 SHALL give Part II §7 its new share.
2. WHEN a task first raises `E0260`, `E0261`, `E0262` or `E0264` THEN that commit SHALL delete the code's row from `crates/piperine-lang/tests/support/code_owners.tsv`, so the guard `spec_codes` passes (`resolved-nets` design D16).

**Independent Test**: `cargo test -p piperine-cli --test docs` and `cargo test -p piperine-lang --test spec_codes`.

---

## Edge Cases

- WHEN a bundle port is left unconnected at the top THEN the system SHALL treat each field as the top's port of that nature (§23.1 rule 1 and L39 apply per field).
- WHEN a field of a one-driver nature has no driver THEN the system SHALL hold its initial value in every analysis, and IF its type does not implement `Default` and no initial value is given THEN the system SHALL report `E0187` (§5.2.8).
- WHEN two bundles of different types have fields of the same names THEN the system SHALL refuse connecting one to a port of the other with `E0251`.
- WHEN an observer view reads every field THEN the system SHALL add no drive.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| BN-01 | P1: A bundle is declared and checked | T1–T4 | In Tasks |
| BN-02 | P1: Views | T5, T6, T9, T11, T12 | In Tasks |
| BN-03 | P1: Connections and literals | T7, T10, T13 | In Tasks |
| BN-04 | P1: Fields are carriers | T6, T8, T12 | In Tasks |
| BN-05 | P2: Functions on a bundle | T17, T18 | In Tasks |
| BN-06 | P2: Nesting, generics and arrays | T14, T15, T19, T20 | In Tasks |
| BN-07 | P2: The host and the POM see the bundle as written | T16 | In Tasks |
| BN-08 | P3: The documents say what is true | T21 | In Tasks |

**Coverage:** 8 total, 8 mapped to tasks (tasks.md, T1–T21), 0 unmapped.

### Gaps closed

| GAPS id | What it says | Closed by |
| ------- | ------------ | --------- |
| L25 | bundles: a `struct` of carriers (BN-01, BN-04), `flip` (BN-02), bundle literals (BN-03), `impl` on a bundle (BN-05) | BN-01 to BN-07 |

## Dependencies

- Phase 1 (BN-01 AC 3 to AC 6 at the declaration): none; POM checks beside `E0263`.
- Phases 2 to 5 (BN-01 AC 1 and AC 2, BN-02, BN-03, BN-04, BN-06 AC 1, BN-07): `composite-values` A–B (on `main`) and `explicit-unknowns` phase A on `main` (`Default`, `E0187`, the fixtures' initial values).
- BN-06 AC 2 (a field of a resolved nature): `resolved-nets` phase 7 (its T22), on the same branch.
- Phase 6 (BN-05): `generics` B (L8 handles, `E0133`) and `unknown-semantics-events` B5 (UE-04, functions that return `Event`).
- Phase 7 (BN-06 AC 3 and AC 4): `generics` A (generic value types) and C (generic modules, the Book's `Slice<T>`); `composite-values` C (L24).
- Phase 8 (BN-08): the Book's natures#8 places `Slice<T>`, a generic module (`generics` C).

## Size

**M.** 21 tasks in 8 phases, 4 batches (tasks.md).

## Sources

- `GAPS.md` §4.2 L25; §9 (Part II §7, 0 %).
- `docs/spec/part_ii_carriers.md` §7 (rules 1 to 15, errors, examples), §5.2; `part_i_values.md` §3 rule 7, §3.1 (handles); `appendix_e_gallery_digital.md` E.6; `appendix_f_gallery_mixed.md` F.3; `appendix_g_scope.md` G.4.
- `.specs/STATE.md` MD-25, MD-54.

---

## Success Criteria

- [ ] No `P0001` remains for a `struct` of carriers used as a type or for `flip`.
- [ ] A producer, a consumer and an observer joined by one bundle run a handshake.
- [ ] `cargo build --workspace` has no warning and `cargo test --workspace` passes.

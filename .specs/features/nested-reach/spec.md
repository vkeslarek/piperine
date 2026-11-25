# Nested Reach Specification

**Approved**: 2026-11-25 - user: "Implement a small feature `nested-reach` in the Piperine repo ...: close GAPS L81 and L82 together ... write `.specs/features/nested-reach/{spec,tasks}.md` (criteria in EARS, one check per criterion with its proof test), then execute." (the delegation names the scope and the form of the criteria; no question is open)

`inst.name` reads through the children of children, at any depth (part III §8.2.8, §8.4.8). Today
a read through one child of a child runs for a node, a param and a `let` over the params
(`bank.c0.sn`), and nothing deeper does: a `pub let` of the grandchild that reads the solve
(`pub let vs = v(sn)`) and a carrier of it are an unnamed `P0001` (GAPS L81), and a path of three
or more names (`p.mid.c0.sn`) is a `P0001` in any order and in a cycle of node connections
(GAPS L82).

## Problem Statement

The elaborator reaches one level below a child by a table of its own (`reach_through_templates.rs`,
`reach_values.rs`) that finds its reads by scanning the design for `a.b.x`, and folds the value
items into constants. A third name, a `let` that reads a node, and a carrier are outside it. A
device author who writes a bank of banks of cells cannot read the cell's storage node, its
`pub let vs = v(sn)` or its state from the top.

## Goals

- [ ] A read through children of children is one mechanism at every depth: each name is one hop, a hop reaches what the module it names lowers for the rest of the path.
- [ ] Every item kind a parent may read (node, carrier, param, `let` over the params, `let` over the solve) is read at any depth, by name, through templates and in generation.
- [ ] A cycle of node connections through such a reach is hardware, at any depth, in any text order.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| A read through a hoisted placement (a module with a result or a value input) that holds a placement of a module that is not hoisted | it has no body of its own to reach into; unchanged, one `P0001` naming the cause (GAPS L95, owner `spec-closure`) |
| A node or carrier declared in generation of a module a parent reads through | its count is known to the module's own lowering only; refused by name as at one level (`own_node_total`), one `P0001` (GAPS L94, owner `spec-closure`) |

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| How the demands of a run are found | By running it: a hop whose demand the run's tables lack records it and the run is made again with it | A scan of the design cannot see what a `pub let` of a grandchild reads, nor the module a template names; the elaboration that reads it can | y - the request: "Make the reach machinery recursive (one path)" |
| What a grandchild's `pub let` is | Lowered in the reach frame of the grandchild, as a `let` of a child is; the printed form of `bank.c0.twice` becomes the lowered `let`, not the folded constant | One path at every depth (§8.2.25: a `pub let` is evaluated in the reader's context) | y - the request: "No special cases" |
| Who owns the cycle at depth | The merge of reached nodes of generics phase D (`place.rs`): a reached slot is a node of the body made before any child is placed, and one node with the child's own once it is | The existing mechanism, applied per hop | y - the request names it |

Open questions: none.

## User Stories

### P1: A node is reached at any depth

**User Story**: As a device author, I want to read the storage node of a cell the top reaches through wrappers, so that a bank of banks of cells needs no flattening by hand.

**Why P1**: L82 and the node half of every other item.

**Acceptance Criteria** (NR-01):

1. WHEN a body reads `v(R.sn)` with `R` a path of two to four names below an instance placed by name THEN the system SHALL reach the cell's own node: the reader's reached node is named by the path and is the node of `Top.<R>`.
2. WHEN a name of the path is placed through a template (`cell: Cell`) THEN the system SHALL reach the node of the module the template names.
3. WHEN a name of the path is placed in generation (`cells[1]`) THEN the system SHALL reach the node of the iteration its index names.
4. WHEN the models and the top are written in either order THEN the system SHALL bind the same nodes and nets.

**Independent Test**: Elaboration tests over nine paths at depth three and four.

---

### P1: A value item is read at any depth (GAPS L81)

**User Story**: As a device author, I want to read what a cell's `pub` items hold, whatever their depth.

**Why P1**: L81.

**Acceptance Criteria** (NR-02):

1. WHEN a body reads a `pub` param or a `pub let` over the params of a cell at depth three or four THEN the system SHALL give it the value of the placement (the iteration's, in generation).
2. WHEN a body reads a `pub let` of the cell that reads the solve (`vs = v(sn)`) at depth three or four THEN the system SHALL evaluate it in the reader's context, reaching the nodes it reads.
3. WHEN a body reads a carrier of the cell (`R.q`) at depth three or four THEN the system SHALL bind a reached carrier of the reader to the cell's net.
4. WHEN the items above are read from source THEN the run SHALL give the values Rust computes for the cell (`sn`, `vs`, `q`, `k`, `twice`), by name, through templates and in generation.
5. WHEN a body drives a resolved carrier of the cell (`p.mid.c0.line = v`) at depth three or four THEN the system SHALL hold the drive beside the cell's own, and the net SHALL resolve both.

**Independent Test**: Elaboration tests, and source runs against Rust's own values.

---

### P1: A cycle of node connections is hardware at any depth (GAPS L82)

**User Story**: As a device author, I want to wire a placement to a node three or four levels down in another placement, or in itself.

**Why P1**: L82's cycle half.

**Acceptance Criteria** (NR-03):

1. WHEN two placements are wired to each other's node at depth three or four, or one to its own THEN the system SHALL bind each port to the node it names, in either text order.
2. WHEN such a design runs THEN the nodes SHALL settle where Kirchhoff's law over the cells' branches puts them, computed by Rust.

**Independent Test**: Elaboration tests of the bindings, source runs of the settled values.

---

### P2: The checker and the visibility rule hold at any depth

**User Story**: As a device author, I want a wrong path refused by name at any depth.

**Why P2**: The refusal side of the same mechanism.

**Acceptance Criteria** (NR-04):

1. WHEN a read of any item kind, or a cycle of connections, is written at depth three or four THEN the checker SHALL type it with no diagnostic.
2. IF a path names an item that is not `pub`, or one the module lacks THEN the system SHALL report `E0343`, at the checker where the path names modules and at elaboration where a template does.

**Independent Test**: Checker tests, and elaboration tests of the refusals.

---

### P2: The records follow

**User Story**: As a reader of `GAPS.md` and the spec, I want the closed gaps gone and the rule written.

**Why P2**: The gap rows and the spec are the contract.

**Acceptance Criteria** (NR-05):

1. WHEN the feature is delivered THEN `GAPS.md` SHALL have no row L81 or L82, section 9 SHALL not name them as left, and the counts SHALL be 17 loud (19 with L94 and L95, the two rows the verification found).
2. WHEN a parent reads through children of children THEN part III §8.2.8 SHALL say that each name of the path is `pub` and the item reads as one level down.

**Independent Test**: `docs.rs` and `spec_text.rs`.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| NR-01 | P1: A node is reached at any depth | T1 | In Tasks |
| NR-02 | P1: A value item is read at any depth | T1, T2 | In Tasks |
| NR-03 | P1: A cycle of node connections is hardware | T1, T2 | In Tasks |
| NR-04 | P2: The checker and the visibility rule | T1 | In Tasks |
| NR-05 | P2: The records follow | T3 | In Tasks |

## Success Criteria

- [ ] `P0001` "a field of something that is not a name bound to a placement" and "an item of a child's child that is not a param or a `let` over its params" are no longer reachable.
- [ ] Every check of tasks.md passes at `HEAD`.

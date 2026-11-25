# Part VI. Input and output

A model writes records, reads files at elaboration, and exposes metadata. None of the three
feeds back into the simulation: no construct of the language reads a record, a run never
reads a file, and metadata is fixed before the first solve.

**Terms defined here:** record, stream, site (of an `emit!`), trace, check, reduction,
reserved stream, metadata, path, gradient param.

**Error codes.** This part owns `E06xx`.

---

## 19. Streams and checks

There is one output statement: `emit!("name", field = value, …)` writes a typed record to the
stream `name`. Every other output is a stdlib function that calls it (§22.6), checks and
`finish()` included.

A **record** is a set of named, typed fields written by one execution of one `emit!`. A
**stream** is the ordered sequence of records written under one name. A **site** is one
`emit!` in the elaborated design: one statement in one instance.

**Syntax**

```
emit           ::= 'emit' '!' '(' STRING ( ',' IDENT '=' expr )* ','? ')'       // an expression of type ()
```

`emit!` is one of the two built-in `!` forms (Appendix A.1); there is no macro definition. The
`STRING` is the stream name, relative to the instance (rule 2). Each `IDENT = expr` is one
named field.

> **Resolution.** The stream name is a string literal, because metadata lists every site with
> its stream (§21).

**Static semantics.**

10. `emit!` is allowed in a reaction and in `analog` (`E0601`). A `fn` that calls it is
    callable only where `emit!` is (§3, Appendix B.2).
11. A field expression has a value type (§1). Field names are distinct within one `emit!`.
12. Within one stream, fields of the same name have the same type at every site.
13. The types and field names of every site are metadata (§21).
14. A site reached from `analog` that writes `sim` or `checks` is `E0603` (rule 4). The check
    follows calls: `require(…)` in `analog`, or in a `fn` called from `analog`, is that error.
15. On the reserved streams, a field the host interprets has the type the stdlib gives it
    (§22.6): on `sim`, `finish: bool`, `failed: bool`, `msg: str`; on `checks`, `kind: str`,
    `name: str`, `pass: bool`, `margin: f64`. Other fields are free.

> **Resolution.** Rules 12 and 15 are stated here.

**Dynamic semantics.**

1. **What a record carries.** Besides its fields, every record carries the instance path of
   its site, the analysis, the instant `(t, n)` and, inside `analog`, the Newton iteration. A
   record is its declared fields and that context, and nothing else (§3.3.1): control takes a
   `bool` (§3.3.5), so a site is reached or it is not.
2. **Names.** Stream names are relative to the instance: `emit!("conv", …)` in `top.adc`
   writes `top.adc.conv`. Two names are a **reserved stream** each, and global: `sim` and
   `checks`. The simulator's own records go to the `log` stream of the instance concerned,
   the one the stdlib `log()` writes (§22.6). Those records are: a loop that does not settle
   (§8.2.3) and a host `force` (§23.1.3).
3. **When.** An output runs when the code around it runs: once per firing in a reaction; once
   per time point in `on settle`; at every evaluation in `analog`. The last includes
   iterations and steps that are later rejected, which is how non-convergence is debugged.
4. **Only reactions decide.** A record written from `analog` depends on the path Newton took.
   It is a **trace** and nothing more. `sim` and `checks` take records from reactions only.
5. **`finish`.** A record on `sim` with `finish = true` ends the analysis at its instant,
   which is the last time point (§17.2.2).
6. **Per analysis.** `emit!` in `analog` runs in equilibrium solves, transient steps and jump
   solves, and does not run in a small-signal sweep (§16.2). `emit!` in a reaction runs
   wherever the reaction does (§16.2, second table).
7. **Rejection.** A record written by a reaction in a step that is later rejected does not
   exist (§17.2.3). A record written by `analog` in such a step stays (rule 3).
8. **Order.** Records of one stream are ordered by instant. Within one microstep they are
   ordered by the instance path of the site, then by source order (§12.1.2). Records of
   the settle phase follow those of every microstep of their time point.
9. **No feedback.** No expression reads a stream. A record has no effect on any carrier,
   solve or event, except rule 5.

> **Resolution.** Rule 5 in detail: the time point completes. Its microstep loop runs until
> nothing is pending and the settle phase runs; then the analysis ends, as at the stop time
> (§15 S6). In a `Dc` sweep the sweep ends at that point. In the operating point of a
> small-signal analysis no sweep is performed.

> **Resolution.** A `finish` record ends the analysis when its `finish` field is true;
> otherwise it is recorded and the analysis goes on. The same holds for `fatal()`.

> **Resolution.** Rules 7 and 8 are stated here. A record from the settle phase carries the
> time point's `t` and the index of its last microstep. Inside `analog`, a `fn` that calls
> `emit!` writes under the path of the instance whose block called it.

Continuing a transient that ended on a `finish` record is open (G.35).

**Checks.** A **check** is a record on `checks`. It is written by an ordinary reaction,
through `require`, `cover` or `target` (§22.6). The language gives a check no meaning; the
host does (§23.2). A `require` record is a pass or a fail: its `pass` field is a `bool`.

**Reductions.** A **reduction** over an analysis (a peak, a count, an integral of samples) is
an explicit signal written by a reaction. There is no end-of-analysis event: the host keeps
each target's last record (§22.6). A settle commit made at the stop time is dropped (§15 S6),
so a reduction reports the value it computed, not the one it stored.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0601` | `emit!` written outside a reaction, `analog` or a `fn`. A call of an emitting `fn` in a wrong position is `E0122` (§3). | elaboration |
| `E0602` | The first argument of `emit!` is not a string literal, or a field is not `IDENT = expr`. | parse |
| `E0603` | A site reached from `analog` writes `sim` or `checks`. | elaboration |
| `E0604` | A field name is repeated in one `emit!`. | elaboration |
| `E0605` | A field's type conflicts with another site of the same stream, or with a reserved stream's field (rule 15). | elaboration |
| `E0606` | A field expression is not of a value type. | elaboration |

**Example**

```rust
module Dut(p: Electrical, n: Electrical, vo: Electrical, vi: Electrical, tj: Thermal) {
    hi: f64 = 1.2;
    lo: f64 = 0.0;
    signal ipk: Bit<f64> = 0.0;          // a reduction is state
}

digital for Dut {
    on settle {
        require("head_hi", v(p, n) <= hi);
        require("head_lo", v(p, n) >= lo);
        target("gain_db", db20(v(vo) / v(vi)) - 40.0);       // a margin: met at or above zero
        cover("hot", temp(tj) > 400.0);
        let pk = ipk.max(p.inflow().abs());
        ipk <- pk;                                           // dropped at the stop time (§15): pk is reported below
        target("ipk_lim", 10m - pk);                         // the host keeps a target's last record (§22.6)
    }
}
```

A message with values in it is a record: `emit!("log", msg = "overrange", vin = v(inp));`.
Fields are typed and formatting them is the host's business. There is no format-string syntax.

---

## 20. Input data

Files are read at elaboration into `const`s; a run never reads a file.

`include_str!(path)` is the text of a file, of type `str`. The stdlib parsers are
`const fn`s over that text (§22.6).

**Syntax**

```
include_str    ::= 'include_str' '!' '(' STRING ')'                            // a const expression of type str
```

`include_str!` is the second built-in `!` form (Appendix A.1).

**Static semantics.**

1. `include_str!` is a `const` expression, allowed in `const` items only (§14).
2. The path is relative to the source file that contains the `include_str!`.
3. The file is read once, at elaboration. Its content is part of the elaborated design. A
   later change to the file has no effect until the host re-elaborates.
4. The file **MUST** be valid UTF-8.
5. The length of a parsed array is the `const`'s type. A file that does not fit that type, in
   its rows or in their number, is an elaboration error.
6. A value read this way is a constant (§3.3.1), not a param, with
   no gradient (§21).

**Dynamic semantics.** None. No file is opened in any analysis, and no primitive reads one
during a run. Data that changes between runs is a param, set by the host (§23).

> **Resolution.** Rules 3, 4 and 6 are stated here. An absolute path is accepted and is read as it is, not relative to the source file.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0610` | `include_str!` used outside a `const` item. | elaboration |
| `E0611` | The argument of `include_str!` is not a string literal. | parse |
| `E0612` | The file cannot be read. The report names the resolved path. | elaboration |
| `E0613` | The file is not valid UTF-8. | elaboration |
| `E0614` | A parsed array does not fit the `const`'s type (rule 5). The report names the file and the row. | elaboration |
| §22.6.4 | A stdlib parser rejects the text (`E0706`). | elaboration |

**Example**

```rust
const IV:  [(f64, f64); 64] = parse_table(include_str!("diode_iv.tbl"));   // whitespace columns
const ROM: [u32; 1024]      = parse_hex(include_str!("boot.hex"));         // $readmemh format

module MeasuredDiode(a: Electrical, k: Electrical) {}
analog for MeasuredDiode { i(a, k) +> table_model(v(a, k), IV, "3E"); }   // cubic, error outside

module BootRom(in addr: Bit<u10>, out data: Bit<u32>) {
    signal mem: [Bit<u32>; 1024] = ROM;
    data = mem[addr];
}
```

---

## 21. Metadata

**Metadata** is what a compiled model tells its host without running. It is fixed at
elaboration.

A **path** is the hierarchical name of an item of the elaborated design:
`top.adc.dac.r[3].r`.

**Static semantics.**

1. A path is the names of the enclosing instances from the top module down, joined by `.`,
   then the item's name. An instance takes the name of the `let` that places it.
2. An item made by a generation `for` is indexed: `cells[3][5]`, one index per enclosing
   loop, outermost first (Appendix F.3).
3. An instance with no `let` of its own is named `<module>#<k>`: the module's name, and `k`
   the 0-based ordinal of the placement among the placements of that module name in the
   enclosing module. Placements are counted in source order of their callee, body and
   blocks alike, then in unrolling order.
4. Metadata does not change during a run. A `set` of a structural param makes the host
   re-elaborate (§8.1.2), which produces new metadata.
5. `pub` decides what other models may use (§8.2). The host can observe everything.
   The `///` text of a quantity of a nature, a reaction, a definition and a generation is
   metadata, read in comment order; the docs of a statement are kept in the tree and have no
   other effect (A.4).

| Item | From | Host operations |
| --- | --- | --- |
| params | the body: type, default, `where`, structural or not, `///` | `list`, `get`, `set` (checked) |
| ports | the port list | `list` |
| public items | `pub` | `list`, probe |
| every node, branch, carrier, drive and instance | the elaborated netlist | probe and record by path, read-only |
| operating-point variables | every named `let` of `analog` (§11 rule 6); an OSDI model's opvars (§8.5) | `get` after an analysis |
| streams and checks | every `emit!` site, with field names and types (§19) | `list`, subscribe before the run |
| gradients | every gradient param | `∂(record)/∂p` after an analysis (§23.3) |

6. The host reads an operating-point variable (§11 rule 6) by path after an analysis. Its
   value is the named `let` evaluated at the last accepted solution: of each sweep point in
   `Dc`, of the operating point in `Ac` and `Noise`.
7. A **gradient param** is an `f64` param that reaches a contribution, a probe-reading
   expression or an event time. The set is found by dependency analysis at elaboration.
8. The run's temperature and multiplicity (§8.5) are no params of an instance: a model's own `m` is listed like any param.
9. An `extern "osdi"` module's params, operating-point variables, noise sources and internal
   nodes appear under the descriptor's names (§8.5).
10. A param that only selects, or only feeds discrete values that are not real, is listed as
    having no gradient (§23.3.6).

> **Resolution.** Rule 3 fixes the spelling, because the order of drives (§4 rule 2) and of
> records (§19 rule 8) is the order of instance paths. `#` cannot occur in an identifier, so
> the name never collides with a `let`. Rule 6's "last accepted solution" and rule 8 are
> stated here.

**Dynamic semantics.** None. Reading metadata runs no model code.

**Errors.** Metadata raises no error of its own. Errors of the host operations in the table
are `E08xx` (§23).

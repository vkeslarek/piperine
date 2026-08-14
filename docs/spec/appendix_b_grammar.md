# Appendix B — Grammar

Piperine's sole frontend is Verilog-AMS (LRM 2.4). Piperine does not maintain its own
EBNF for that grammar — the authoritative, complete grammar is the LRM's own **Annex A**
(`VAMS-LRM-2-4.md` at the repo root, `# **Annex A**` through `# **Annex B**`, currently
lines 16560–17959 of the in-repo copy). This appendix does not restate it. What follows
is (B.1) how conformance to Annex A is mechanically checked, and (B.2) the one place
Piperine's own semantics ride on top of standard Annex A syntax rather than inside it.

## B.1 Conformance gate

Two test targets in `crates/piperine-lang/tests/` hold the parser to the LRM, both
re-deriving their ground truth from the checked-in `VAMS-LRM-2-4.md` at test time rather
than from a frozen copy — so the gate can't quietly drift from the document it claims to
cover.

### B.1.1 `annex_a_grammar.rs` — grammar section coverage

A table (`SECTIONS`) lists every LRM Annex A section at **leaf granularity** (down to
e.g. A.2.1.1/A.2.1.2/A.2.1.3, skipping only bare parent headings like A.2.1 that carry no
direct content). Each section is marked one of:

- `Implemented` — some `grammar/{mod,cursor,expr,stmt,item}.rs` production's doc comment
  cites it (`Annex A.X.Y`); a test asserts the citation string is actually present in the
  concatenated grammar source.
- `OutOfScope(reason)` — not yet implemented, with a **mandatory, non-blank** written
  reason naming what covers it and when (a task/phase reference). A blank reason fails
  the gate outright.

A third test cross-checks the table itself against the document: every `### **A.X.Y**` /
`#### **A.X.Y.Z**` leaf heading found inside Annex A's span in `VAMS-LRM-2-4.md` must have
a matching row in `SECTIONS`, so a future LRM section can't go silently unaccounted for
either by omission from the table or by a stale citation.

The granularity choice is a recorded `SPEC_DEVIATION`: a literal per-production regex
over the LRM's PDF-to-text extraction produces ~500 noisy, often-merged pseudo-productions
that don't correspond to real grammar rules; section headers are the unit the source
document actually supports mechanically, and the same unit `grammar/*.rs` doc comments
already cite.

As of this writing, notable `OutOfScope` areas include: `config_declaration` bodies
(A.1.5), `connectrules` semantics beyond the declaration shell (A.1.8), the full IEEE 1364
net-type/strength/delay tables and primitive/UDP truth-table semantics (A.2.2.x, A.3.x,
A.5.4), specify-block internals treated as an opaque span (A.7.2–A.7.5.3), and a handful
of sections whose grammar is implemented but cited under a sibling section to avoid
double-booking one method across two rows (e.g. A.8.2/A.8.4/A.8.5/A.8.6 fold into A.8.3's
Pratt-parser citation; A.8.7/A.8.8/A.9.2/A.9.4 are the lexer's scope, cited against the
LRM's own numbered subsections instead of Annex A). None of this is guessed — read
`annex_a_grammar.rs`'s `SECTIONS` table for the current, authoritative list; it changes as
phases land.

### B.1.2 `annex_b_keywords.rs` — reserved word set

Independently re-derives the keyword set from `VAMS-LRM-2-4.md`'s own **Table B.1** (the
"List of keywords" table, scanned for backtick-delimited lowercase cells between the
`Table B.1` caption and the following `Analog language subset` heading) and diffs it
against `Keywords::all()` in `keywords.rs` — any keyword present in one but not the other
fails the gate. Three further tests hold every keyword in that set to §2.8.2's rule that
a keyword is lexically a plain identifier, distinguished only at the grammar layer:
`Keywords::is_reserved` is true for all of them, each one lexes as a bare `Tok::Ident`,
and each is rejected as an identifier with a typed `ParseErrorKind::UnexpectedToken`
naming the keyword — never a silent fallback.

## B.2 Piperine's own surface: `device`/`port` attributes

Piperine's plugin system needs two pieces of metadata that Annex A has no dedicated
grammar for: which plugin implements a module as a native device, and how a port maps to
that device's terminal model. These ride entirely on **standard** Annex A.9.1
`attribute_instance` syntax (`(* attr_spec {, attr_spec} *)`, `attr_spec ::= attr_name [
"=" constant_expression ]`) — parsed once, generically, by
`grammar::cursor::Cursor::parse_attr_instances` (cited against Annex A.9.1) and shared by
every attribute-bearing site in the grammar. **There is no syntactic extension**: no new
production, no nested-call form, nothing the LRM parser doesn't already accept. What's
Piperine-specific is purely semantic — which attribute *names* are recognized, and what
they mean — decided during elaboration, not during parsing.

`crates/piperine-lang/src/elab/lower/vams.rs` (`lower_module_attrs`/`lower_port_attrs`,
task T35) is where this recognition happens:

| Attribute names (sibling `attr_spec`s in one instance) | Recognized on | Elaborates to |
|---|---|---|
| `device = "<plugin>"`, `type = "<type-id>"` | a module declaration | plugin-system `device` schema (`Attribute { schema: "device", data: { plugin, type } }`) |
| `port = "<name>"`, `kind = "<kind>"` (optional) | a port declaration | plugin-system `port` schema (`Attribute { schema: "port", data: { name, kind? } }`) |

Because the LRM's `attr_spec` has no nested-call form, the schema's two (or three) fields
travel as separate sibling attribute names inside one `(* … *)` instance —
`(* device = "some_plugin", type = "diode_v2" *)` — rather than as
`(* device(plugin = …, type = …) *)`; this is a recorded `SPEC_DEVIATION` in
`lower_module_attrs`'s doc comment. `type`/`kind` are only consumed once their sibling
`device`/`port` attribute is present; standing alone they're just attribute names subject
to the same check as everything else. Any attribute name on a module or port that is
neither `device`/`type`/`port`/`kind` nor a name the plugin system has itself registered
in `known_schemas` fails loud with `ElabErrorKind::UnknownAttrSchema` (MD-24's
no-silent-fallback rule, ported to VAMS as requirement VAMS-13) — an unrecognized
attribute is a compile error, never inert. These two schemas are seeded by the plugin
system itself (`PluginHost::seed_schemas`); individual plugins contribute no schemas of
their own (see CLAUDE.md, "Declared language surface").

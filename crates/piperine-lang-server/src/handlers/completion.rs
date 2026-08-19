//! Completion handler: keyword/snippet and `@schema` completions.
//!
//! **T37 (MD-45) regression, documented**: the PHDL-era predictive parser
//! (`piperine_lang::parse::predict`, `ExpectedSyntax`/`IdentRole`) this
//! handler used for cursor-context-aware completion (only suggest what
//! the grammar actually accepts at the cursor) was deleted with `piperine-
//! lang/src/parse/` at T21 — it was hand-written specifically against
//! PHDL's own grammar shape, not mechanically portable, and rebuilding an
//! equivalent tolerant/predictive parser for VAMS's grammar is new
//! architecture work (a second parsing mode for `piperine-vams`), not a
//! retarget. `completions_at` now offers a static, non-context-aware set
//! of VAMS keywords/snippets/operators/sysfuncs (still filtered by the
//! BUG-4 heuristic where it can — see below) plus `@schema` completion
//! (T18/LSP-20, unaffected — that path never used the predictive parser).

use lsp_server::{Connection, Request};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionParams,
    CompletionResponse, InsertTextFormat,
};

use super::{ConnectionExt, RequestExt};
use crate::state::{DocumentState, ServerState};
use piperine_lang::elab::registry::ElabContext;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<CompletionParams>(connection) else { return };

    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;

    let items = state
        .documents
        .get(&uri)
        .map(|doc| {
            let offset = crate::text_pos::position_to_byte(&doc.source, pos);
            completions_at(doc, offset)
        })
        .unwrap_or_default();

    let result = CompletionResponse::List(CompletionList { is_incomplete: false, items });
    connection.respond(id, result);
}

/// The completion items for `offset` in `doc` — either `@schema` completion
/// (T18/LSP-20, when the cursor sits right after an `@` and an optional
/// partial schema name) or the static, non-context-aware keyword/snippet
/// set (T37: no predictive parser exists for VAMS, see this module's
/// header doc). Exposed as its own function (rather than inlined in
/// `handle`) so tests can drive it directly against a `DocumentState`
/// without a full LSP round trip.
pub fn completions_at(doc: &DocumentState, offset: usize) -> Vec<CompletionItem> {
    if let Some(prefix) = attr_schema_prefix(&doc.source, offset) {
        return schema_completions(doc.ctx.as_ref(), prefix);
    }
    if outside_any_module(&doc.source, offset) {
        // T37's note stands (no predictive parser); the one bit of
        // context the static list can honor cheaply is the module-body
        // boundary — a cursor outside every module body (after the last
        // `endmodule`, or before the first `module`) must not be offered
        // statement keywords.
        return top_level_completions(doc.design.as_ref());
    }
    static_completions(doc.design.as_ref())
}

/// Whether `offset` sits outside every module body — before the first
/// `module` keyword or after the last `endmodule` (a cursor between two
/// modules is outside both). A lexical scan, not a parse: comments could
/// confuse it, and that is the disclosed price of T37's static-list
/// architecture (the honest alternative is a tolerant parser, a named
/// follow-up).
fn outside_any_module(source: &str, offset: usize) -> bool {
    let before = &source[..offset.min(source.len())];
    let opens = before.matches("module ").count() + before.matches("module(").count();
    let closes = before.matches("endmodule").count();
    opens <= closes
}

/// The top-level-only half of [`static_completions`]: declaration
/// keywords, snippets, declared discipline/module names — no statement
/// keywords, no value types.
fn top_level_completions(design: Option<&piperine_lang::Design>) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    add_top_level_completions(&mut items);
    if let Some(design) = design {
        for (name, _) in design.disciplines() {
            items.push(kw_item(name, "Discipline", CompletionItemKind::INTERFACE));
        }
        for m in design.modules() {
            items.push(kw_item(m.name(), "Module", CompletionItemKind::CLASS));
        }
    }
    items.sort_by(|a, b| a.label.cmp(&b.label));
    items.dedup_by(|a, b| a.label == b.label);
    items
}

/// Whether `offset` sits right after an `@` (possibly with a partial
/// identifier already typed, e.g. `@rf|`) — an attribute-schema-name
/// completion position. Returns the partial identifier typed so far (empty
/// string when the cursor is immediately after the bare `@`).
fn attr_schema_prefix(source: &str, offset: usize) -> Option<&str> {
    let bytes = source.as_bytes();
    if offset > bytes.len() {
        return None;
    }
    let mut i = offset;
    while i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_') {
        i -= 1;
    }
    if i > 0 && bytes[i - 1] == b'@' {
        Some(&source[i..offset])
    } else {
        None
    }
}

/// Only the in-scope schema names (`ctx.schemas` — T35's
/// `HashSet<String>` seam) whose name starts with `prefix`, as
/// completion items (T18/LSP-20).
fn schema_completions(ctx: Option<&ElabContext>, prefix: &str) -> Vec<CompletionItem> {
    let Some(ctx) = ctx else { return Vec::new() };
    let mut items: Vec<CompletionItem> = ctx
        .schemas
        .iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| kw_item(name, "Attribute schema", CompletionItemKind::PROPERTY))
        .collect();
    items.sort_by(|a, b| a.label.cmp(&b.label));
    items
}

/// The static, non-context-aware completion set (T37 — see this module's
/// header doc for why there is no cursor-context filtering anymore):
/// every keyword/snippet/operator/sysfunc list this file declares, plus
/// every discipline/module name the current document's `Design` (if any)
/// declares.
pub fn static_completions(design: Option<&piperine_lang::Design>) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    add_top_level_completions(&mut items);
    add_behavior_completions(&mut items);
    add_value_types(&mut items);
    add_events(&mut items);
    add_sysfuncs(&mut items);

    if let Some(design) = design {
        for (name, _) in design.disciplines() {
            items.push(kw_item(name, "Discipline", CompletionItemKind::INTERFACE));
        }
        for m in design.modules() {
            items.push(kw_item(m.name(), "Module", CompletionItemKind::CLASS));
        }
    }

    // Deduplicate items based on label
    items.sort_by(|a, b| a.label.cmp(&b.label));
    items.dedup_by(|a, b| a.label == b.label);

    items
}

fn add_top_level_completions(items: &mut Vec<CompletionItem>) {
    // VAMS Annex B top-level-declaration keywords (T37 — swapped from the
    // PHDL-only `bundle`/`capability`/`impl`/`pub`/`const` set, none of
    // which VAMS has).
    let keywords = [
        ("module", "Module declaration", CompletionItemKind::KEYWORD),
        ("discipline", "Discipline declaration", CompletionItemKind::KEYWORD),
        ("nature", "Nature declaration", CompletionItemKind::KEYWORD),
        ("analog", "Analog behavior block", CompletionItemKind::KEYWORD),
        ("always", "Digital always block", CompletionItemKind::KEYWORD),
        ("initial", "Initial block", CompletionItemKind::KEYWORD),
        ("branch", "Branch declaration", CompletionItemKind::KEYWORD),
        ("genvar", "Generate-loop index declaration", CompletionItemKind::KEYWORD),
        ("parameter", "Parameter declaration", CompletionItemKind::KEYWORD),
    ];
    for (kw, desc, kind) in &keywords {
        items.push(kw_item(kw, desc, *kind));
    }

    // Snippets
    items.push(CompletionItem {
        label: "module".into(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Module snippet".into()),
        insert_text: Some("module ${1:Name} (${2:ports});\n\t$0\nendmodule".into()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    });
    items.push(CompletionItem {
        label: "analog".into(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Analog block snippet".into()),
        insert_text: Some("analog begin\n\t$0\nend".into()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    });
    items.push(CompletionItem {
        label: "discipline".into(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Discipline snippet".into()),
        insert_text: Some("discipline ${1:Name};\n\tpotential ${2:V};\n\tflow ${3:I};\nenddiscipline".into()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    });
    items.push(CompletionItem {
        label: "nature".into(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Nature snippet".into()),
        insert_text: Some("nature ${1:Name};\n\tunits = \"${2:V}\";\n\taccess = ${3:V};\n\tabstol = ${4:1e-6};\nendnature".into()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    });
}

fn add_behavior_completions(items: &mut Vec<CompletionItem>) {
    let keywords = [
        ("if", "Conditional statement", CompletionItemKind::KEYWORD),
        ("for", "For loop (generate / procedural)", CompletionItemKind::KEYWORD),
        ("case", "Case statement", CompletionItemKind::KEYWORD),
        ("assign", "Continuous assignment", CompletionItemKind::KEYWORD),
    ];
    for (kw, desc, kind) in &keywords {
        items.push(kw_item(kw, desc, *kind));
    }

    let operators = [
        ("V", "Branch potential access"),
        ("I", "Branch flow access"),
        ("ddt", "Time derivative"),
        ("idt", "Time integral"),
        ("delay", "Delay"),
        ("transition", "Transition filter"),
        ("slew", "Slew-rate filter"),
    ];
    for (op, desc) in &operators {
        items.push(CompletionItem {
            label: op.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(desc.to_string()),
            insert_text: Some(format!("{op}($1)")),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }
}

fn add_value_types(items: &mut Vec<CompletionItem>) {
    // LRM §3.2's own type keywords (lowercase — the PascalCase set was
    // the PHDL value layer, none of which VAMS has).
    let types = [
        ("real", "Real variable/parameter type"),
        ("integer", "Integer variable/parameter type"),
        ("string", "String parameter type"),
        ("time", "Time variable type"),
        ("realtime", "Realtime variable type"),
    ];
    for (ty, desc) in &types {
        items.push(CompletionItem {
            label: ty.to_string(),
            kind: Some(CompletionItemKind::TYPE_PARAMETER),
            detail: Some(desc.to_string()),
            ..Default::default()
        });
    }
}

fn add_events(items: &mut Vec<CompletionItem>) {
    let events = [
        ("posedge", "Rising edge trigger"),
        ("negedge", "Falling edge trigger"),
        ("change", "Value change trigger"),
        ("cross", "Analog crossing trigger"),
        ("initial", "Initial step trigger"),
    ];
    for (ev, desc) in &events {
        items.push(kw_item(ev, desc, CompletionItemKind::EVENT));
    }
}

fn add_sysfuncs(items: &mut Vec<CompletionItem>) {
    let sys_funcs = [
        ("$temperature", "Simulation temperature"),
        ("$abstime", "Absolute simulation time"),
        ("$finish", "End simulation"),
        ("$display", "Print message"),
    ];
    for (sf, desc) in &sys_funcs {
        items.push(CompletionItem {
            label: sf.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(desc.to_string()),
            insert_text: Some(format!("{sf}($1)")),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }
}

fn kw_item(label: &str, detail: &str, kind: CompletionItemKind) -> CompletionItem {
    CompletionItem {
        label: label.into(),
        kind: Some(kind),
        detail: Some(detail.into()),
        ..Default::default()
    }
}

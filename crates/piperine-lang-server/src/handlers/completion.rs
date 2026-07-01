//! Completion handler: provides keyword and context-aware completions.
//!
//! Returns PHDL keywords, built-in types, and context-sensitive completions
//! (discipline names, module names for instances, port/param/wire names).

use lsp_server::{Connection, Request, Response};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionParams,
    CompletionResponse, InsertTextFormat, Position,
};
use serde_json::from_value;

use crate::state::ServerState;

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let (id, params) = (req.id, req.params);
    let params = match from_value::<CompletionParams>(params) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("invalid completion params: {e}");
            return;
        }
    };

    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;

    let items = state
        .documents
        .get(&uri)
        .map(|doc| {
            let ctx = detect_context(&doc.source, pos);
            build_completions(ctx, doc.design.as_ref())
        })
        .unwrap_or_default();

    let result = CompletionResponse::List(CompletionList { is_incomplete: true, items });

    let response = Response {
        id,
        result: Some(serde_json::to_value(result).unwrap()),
        error: None,
    };
    connection
        .sender
        .send(lsp_server::Message::Response(response))
        .unwrap();
}

/// What kind of syntactic context the cursor is in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompletionContext {
    /// Top-level: module, discipline, bundle, enum, capability, fn, impl, use, pub.
    TopLevel,
    /// Inside a `mod Name { ... }` body: param, wire, var, for, if, instance.
    ModBody,
    /// Inside an `analog { ... }` or `digital { ... }` or `fn { ... }` block.
    Behavior,
}

/// Heuristic: scan backwards from position to detect what scope we're in.
pub fn detect_context(source: &str, position: Position) -> CompletionContext {
    let prefix = position_to_prefix(source, position);

    // Look at the last 500 chars for keyword context.
    let recent = if prefix.len() > 500 { &prefix[prefix.len() - 500..] } else { prefix };
    if recent.contains("analog ") || recent.contains("digital ") {
        return CompletionContext::Behavior;
    }
    if recent.contains("mod ") {
        // Check if we're inside braces of a mod (not just passed a mod keyword).
        let open_braces = recent.matches('{').count();
        let close_braces = recent.matches('}').count();
        if open_braces > close_braces {
            return CompletionContext::ModBody;
        }
    }
    // Check if we're inside a function body.
    if recent.contains("fn ") {
        let open_braces = recent.matches('{').count();
        let close_braces = recent.matches('}').count();
        if open_braces > close_braces {
            return CompletionContext::Behavior;
        }
    }
    CompletionContext::TopLevel
}

fn position_to_prefix(source: &str, position: Position) -> &str {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, c) in source.char_indices() {
        if line == position.line && col == position.character {
            return &source[..i];
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    source
}

pub fn build_completions(ctx: CompletionContext, design: Option<&piperine_lang::Design>) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    match ctx {
        CompletionContext::TopLevel => add_top_level_completions(&mut items),
        CompletionContext::ModBody => add_mod_body_completions(&mut items),
        CompletionContext::Behavior => add_behavior_completions(&mut items),
    }

    // Always add value types and system functions.
    add_value_types(&mut items);
    add_events(&mut items);
    add_sysfuncs(&mut items);

    // Context-aware: discipline, module, function, capability names from design.
    if let Some(design) = design {
        add_design_names(&mut items, design, ctx);
    }

    items
}

fn add_top_level_completions(items: &mut Vec<CompletionItem>) {
    let keywords = [
        ("mod", "Module declaration", CompletionItemKind::KEYWORD),
        ("fn", "Function declaration", CompletionItemKind::KEYWORD),
        ("analog", "Analog behavior block", CompletionItemKind::KEYWORD),
        ("digital", "Digital behavior block", CompletionItemKind::KEYWORD),
        ("discipline", "Discipline declaration", CompletionItemKind::KEYWORD),
        ("bundle", "Bundle (struct) declaration", CompletionItemKind::KEYWORD),
        ("enum", "Enum declaration", CompletionItemKind::KEYWORD),
        ("capability", "Capability (trait) declaration", CompletionItemKind::KEYWORD),
        ("impl", "Impl block", CompletionItemKind::KEYWORD),
        ("pub", "Public visibility", CompletionItemKind::KEYWORD),
        ("use", "Import declaration", CompletionItemKind::KEYWORD),
    ];
    for (kw, desc, kind) in &keywords {
        items.push(kw_item(kw, desc, *kind));
    }
}

fn add_mod_body_completions(items: &mut Vec<CompletionItem>) {
    let keywords = [
        ("param", "Parameter declaration", CompletionItemKind::KEYWORD),
        ("wire", "Wire (net) declaration", CompletionItemKind::KEYWORD),
        ("var", "Variable declaration", CompletionItemKind::KEYWORD),
        ("for", "Structural for loop", CompletionItemKind::KEYWORD),
        ("if", "Structural if", CompletionItemKind::KEYWORD),
        ("input", "Input direction", CompletionItemKind::KEYWORD),
        ("output", "Output direction", CompletionItemKind::KEYWORD),
        ("inout", "Bidirectional direction", CompletionItemKind::KEYWORD),
    ];
    for (kw, desc, kind) in &keywords {
        items.push(kw_item(kw, desc, *kind));
    }
}

fn add_behavior_completions(items: &mut Vec<CompletionItem>) {
    let keywords = [
        ("var", "Variable declaration", CompletionItemKind::KEYWORD),
        ("if", "Conditional statement", CompletionItemKind::KEYWORD),
        ("for", "For loop", CompletionItemKind::KEYWORD),
        ("match", "Match expression", CompletionItemKind::KEYWORD),
        ("return", "Return statement", CompletionItemKind::KEYWORD),
        ("when", "Event guard", CompletionItemKind::KEYWORD),
    ];
    for (kw, desc, kind) in &keywords {
        items.push(kw_item(kw, desc, *kind));
    }

    // Analog operators.
    let operators = [
        ("V", "Branch potential access"),
        ("I", "Branch flow access"),
        ("ddt", "Time derivative"),
        ("idt", "Time integral"),
        ("idtmod", "Modulo time integral"),
        ("ddx", "Spatial derivative"),
        ("delay", "Delay"),
        ("absdelay", "Absolute delay"),
        ("transition", "Transition filter"),
        ("slew", "Slew-rate filter"),
        ("laplace_np", "Laplace transform"),
        ("laplace_zp", "Laplace transform (zero-pole)"),
        ("white_noise", "White noise source"),
        ("flicker_noise", "Flicker noise source"),
        ("ac_stim", "AC stimulus"),
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
    let types = [
        ("Real", "Real number (f64)"),
        ("Natural", "Natural number (u64)"),
        ("Integer", "Integer (i64)"),
        ("Complex", "Complex number"),
        ("Boolean", "Boolean"),
        ("Quad", "4-valued logic"),
        ("String", "String"),
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
        ("above", "Analog threshold trigger"),
        ("initial", "Initial step trigger"),
        ("final", "Final step trigger"),
    ];
    for (ev, desc) in &events {
        items.push(kw_item(ev, desc, CompletionItemKind::EVENT));
    }
}

fn add_sysfuncs(items: &mut Vec<CompletionItem>) {
    let sys_funcs = [
        ("$temperature", "Simulation temperature"),
        ("$vt", "Thermal voltage"),
        ("$abstime", "Absolute simulation time"),
        ("$mfactor", "Multiplicity factor"),
        ("$bound_step", "Limit time step"),
        ("$discontinuity", "Signal discontinuity"),
        ("$finish", "End simulation"),
        ("$display", "Print message"),
        ("$strobe", "Print at end of timestep"),
        ("$limit", "Limit function"),
        ("$random", "Random number"),
        ("$simparam", "Simulation parameter"),
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

fn add_design_names(
    items: &mut Vec<CompletionItem>,
    design: &piperine_lang::Design,
    _ctx: CompletionContext,
) {
    for (name, _) in design.disciplines() {
        items.push(CompletionItem {
            label: name.clone(),
            kind: Some(CompletionItemKind::CLASS),
            detail: Some("discipline".into()),
            ..Default::default()
        });
    }
    for m in design.modules() {
        items.push(CompletionItem {
            label: m.name().to_string(),
            kind: Some(CompletionItemKind::CLASS),
            detail: Some(format!("module ({} ports)", m.ports().len())),
            ..Default::default()
        });
    }
    for f in design.functions() {
        items.push(CompletionItem {
            label: f.name().to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some("function".into()),
            ..Default::default()
        });
    }
    for (name, _) in design.capabilities() {
        items.push(CompletionItem {
            label: name.clone(),
            kind: Some(CompletionItemKind::INTERFACE),
            detail: Some("capability".into()),
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

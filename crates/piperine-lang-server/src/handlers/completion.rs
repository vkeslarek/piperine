//! Completion handler: provides keyword and context-aware completions using piperine-lang predictive parsing.

use lsp_server::{Connection, Request, Response};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionParams,
    CompletionResponse, InsertTextFormat, Position,
};
use serde_json::from_value;

use crate::state::ServerState;
use piperine_lang::parse::predict::{ExpectedSyntax, IdentRole};

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
            let offset = position_to_offset(&doc.source, pos);
            let expected = piperine_lang::parse::predict_at_cursor(&doc.source, offset);
            build_completions_predictive(&expected, doc.design.as_ref())
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

fn position_to_offset(source: &str, position: Position) -> usize {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, c) in source.char_indices() {
        if line == position.line && col == position.character {
            return i;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    source.len()
}

pub fn build_completions_predictive(expected: &[ExpectedSyntax], design: Option<&piperine_lang::Design>) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    for req in expected {
        match req {
            ExpectedSyntax::Keyword(kw) => {
                items.push(kw_item(kw, "Keyword", CompletionItemKind::KEYWORD));
            }
            ExpectedSyntax::Ident(role) => match role {
                IdentRole::TypeName => {
                    add_value_types(&mut items);
                    if let Some(design) = design {
                        for (name, _) in design.disciplines() {
                            items.push(kw_item(name, "Discipline", CompletionItemKind::INTERFACE));
                        }
                    }
                }
                IdentRole::ModName => {
                    if let Some(design) = design {
                        for m in design.modules() {
                            items.push(kw_item(m.name(), "Module", CompletionItemKind::CLASS));
                        }
                    }
                }
                IdentRole::CapabilityName => {
                    if let Some(design) = design {
                        for (name, _) in design.capabilities() {
                            items.push(kw_item(name, "Capability", CompletionItemKind::INTERFACE));
                        }
                    }
                }
                _ => {}
            },
            ExpectedSyntax::Expression => {
                add_behavior_completions(&mut items);
                add_sysfuncs(&mut items);
                add_events(&mut items);
            }
            ExpectedSyntax::Punctuation(_) => {}
        }
    }
    
    // Fallback: If the parser returned absolutely nothing (which is rare but possible during early typing),
    // we can provide some top-level defaults if the document is empty.
    if expected.is_empty() {
        add_top_level_completions(&mut items);
    }
    
    // Deduplicate items based on label
    items.sort_by(|a, b| a.label.cmp(&b.label));
    items.dedup_by(|a, b| a.label == b.label);
    
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

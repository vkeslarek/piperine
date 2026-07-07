//! Completion handler: provides keyword and context-aware completions using piperine-lang predictive parsing.

use lsp_server::{Connection, Request};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionParams,
    CompletionResponse, InsertTextFormat,
};

use super::{ConnectionExt, RequestExt};
use crate::state::ServerState;
use piperine_lang::parse::predict::{ExpectedSyntax, IdentRole};

pub fn handle(state: &mut ServerState, req: Request, connection: &Connection) {
    let Some((id, params)) = req.parse::<CompletionParams>(connection) else { return };

    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;

    let items = state
        .documents
        .get(&uri)
        .map(|doc| {
            let offset = crate::text_pos::position_to_byte(&doc.source, pos);
            let expected = piperine_lang::parse::predict_at_cursor(&doc.source, offset);
            build_completions_predictive(&expected, doc.design.as_ref())
        })
        .unwrap_or_default();

    let result = CompletionResponse::List(CompletionList { is_incomplete: false, items });
    connection.respond(id, result);
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
        ("fn", "Function declaration", CompletionItemKind::KEYWORD),
        ("digital", "Digital behavior block", CompletionItemKind::KEYWORD),
        ("bundle", "Bundle (struct) declaration", CompletionItemKind::KEYWORD),
        ("enum", "Enum declaration", CompletionItemKind::KEYWORD),
        ("capability", "Capability (trait) declaration", CompletionItemKind::KEYWORD),
        ("impl", "Impl block", CompletionItemKind::KEYWORD),
        ("pub", "Public visibility", CompletionItemKind::KEYWORD),
        ("use", "Import declaration", CompletionItemKind::KEYWORD),
        ("const", "Constant declaration", CompletionItemKind::KEYWORD),
    ];
    for (kw, desc, kind) in &keywords {
        items.push(kw_item(kw, desc, *kind));
    }
    
    // Snippets
    items.push(CompletionItem {
        label: "mod".into(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Module snippet".into()),
        insert_text: Some("mod ${1:Name} {\n\t$0\n}".into()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    });
    items.push(CompletionItem {
        label: "analog".into(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Analog block snippet".into()),
        insert_text: Some("analog {\n\t$0\n}".into()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    });
    items.push(CompletionItem {
        label: "bench".into(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Bench block snippet".into()),
        insert_text: Some("bench ${1:Name} {\n\t$0\n}".into()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    });
    items.push(CompletionItem {
        label: "discipline".into(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Discipline snippet".into()),
        insert_text: Some("discipline ${1:Name} {\n\tdomain: ${2:continuous},\n\t$0\n}".into()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    });
    items.push(CompletionItem {
        label: "bundle".into(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Bundle snippet".into()),
        insert_text: Some("bundle ${1:Name} {\n\t$0\n}".into()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    });
    items.push(CompletionItem {
        label: "capability".into(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Capability snippet".into()),
        insert_text: Some("capability ${1:Name} {\n\tfn ${2:method}(self, ${3:args}) -> ${4:RetType};\n}".into()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    });
    items.push(CompletionItem {
        label: "impl".into(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Impl snippet".into()),
        insert_text: Some("impl ${1:Capability} for ${2:Type} {\n\tfn ${3:method}(self, ${4:args}) -> ${5:RetType} {\n\t\t$0\n\t}\n}".into()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    });
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

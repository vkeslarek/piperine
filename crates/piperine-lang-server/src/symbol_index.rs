use piperine_lang::pom::Design;
use miette::SourceSpan;

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Module,
    Port,
    Param,
    Wire,
    Var,
    Instance,
    Function,
    Behavior,
    Bench,
    Enum,
    Bundle,
    Discipline,
    Capability,
}

#[derive(Debug, Clone)]
pub struct Resolution {
    pub kind: SymbolKind,
    pub name: String,
    pub decl_span: Option<SourceSpan>,
    pub type_info: Option<String>,
}

pub fn resolve_at(design: &Design, source: &str, byte_offset: usize) -> Option<Resolution> {
    // 1. Identify what we are hovering over.
    // For now, we just find the word under the cursor.
    let word = crate::text_pos::word_at_position(
        source,
        crate::text_pos::byte_to_position(source, byte_offset),
    )?;

    // 2. Global lookup for now (until we build true scope resolution)
    // to keep the handlers working but using the new Resolution API.
    for m in design.modules() {
        if m.name == word {
            return Some(Resolution {
                kind: SymbolKind::Module,
                name: m.name.clone(),
                decl_span: m.span,
                type_info: None,
            });
        }
        for p in &m.ports {
            if p.name == word {
                return Some(Resolution {
                    kind: SymbolKind::Port,
                    name: p.name.clone(),
                    decl_span: p.span,
                    type_info: Some(format!("{:?}", p.direction)), // Basic type info
                });
            }
        }
        for p in &m.params {
            if p.name == word {
                return Some(Resolution {
                    kind: SymbolKind::Param,
                    name: p.name.clone(),
                    decl_span: p.span,
                    type_info: Some(format!("{:?}", p.ty)),
                });
            }
        }
        for w in &m.wires {
            if w.name == word {
                return Some(Resolution {
                    kind: SymbolKind::Wire,
                    name: w.name.clone(),
                    decl_span: w.span,
                    type_info: Some(format!("{:?}", w.ty)),
                });
            }
        }
        for v in &m.vars {
            if v.name == word {
                return Some(Resolution {
                    kind: SymbolKind::Var,
                    name: v.name.clone(),
                    decl_span: v.span,
                    type_info: Some(format!("{:?}", v.ty)),
                });
            }
        }
        for i in &m.instances {
            if i.label.as_deref() == Some(&word) || i.module == word {
                return Some(Resolution {
                    kind: SymbolKind::Instance,
                    name: i.label.clone().unwrap_or_else(|| i.module.clone()),
                    decl_span: i.span,
                    type_info: Some(format!("instance of {}", i.module)),
                });
            }
        }
        for b in &m.behaviors {
            if b.name == word {
                return Some(Resolution {
                    kind: SymbolKind::Behavior,
                    name: b.name.clone(),
                    decl_span: b.span,
                    type_info: Some(format!("{:?}", b.kind)),
                });
            }
        }
    }
    
    for b in design.benches() {
        if b.module == word {
            return Some(Resolution {
                kind: SymbolKind::Bench,
                name: b.module.clone(),
                decl_span: b.span,
                type_info: None,
            });
        }
    }

    for (name, e) in design.enums() {
        if *name == word {
            return Some(Resolution {
                kind: SymbolKind::Enum,
                name: name.clone(),
                decl_span: e.span,
                type_info: None,
            });
        }
    }

    for (name, b) in design.bundles() {
        if *name == word {
            return Some(Resolution {
                kind: SymbolKind::Bundle,
                name: name.clone(),
                decl_span: b.span,
                type_info: None,
            });
        }
    }

    for (name, d) in design.disciplines() {
        if *name == word {
            return Some(Resolution {
                kind: SymbolKind::Discipline,
                name: name.clone(),
                decl_span: d.span,
                type_info: None,
            });
        }
    }

    for (name, c) in design.capabilities() {
        if *name == word {
            return Some(Resolution {
                kind: SymbolKind::Capability,
                name: name.clone(),
                decl_span: c.span,
                type_info: None,
            });
        }
    }

    for i in design.impls() {
        for m in &i.methods {
            if m.name == word {
                return Some(Resolution {
                    kind: SymbolKind::Function,
                    name: format!("{}::{}", i.ty, m.name),
                    decl_span: m.span,
                    type_info: Some(format!("impl method for {}", i.ty)),
                });
            }
        }
    }

    None
}

//! Stage 8: structural checks over declarations alone, with no types and no
//! const evaluation: the shape of an `extern` module (`E0370`), the body rule
//! of functions (`E0125`) and the fields of a nature (`E0201`). Runs after
//! classification: a value input is a port the classifier made one.

use super::super::decl::DeclKind;
use super::super::design::Design;
use super::super::ids::{DeclId, FileId};
use super::super::resolution::{NatureKind, PortKind};
use crate::msl::Diagnostic;
use crate::msl::ast::{Lit, LitKind};

/// Every structural error of the design, in declaration order.
pub(super) fn run(design: &Design) -> Vec<(FileId, Diagnostic)> {
    let mut out = Vec::new();
    for i in 0..design.decl_count() {
        let id = DeclId(i as u32);
        let data = design.decl_data(id);
        match &data.kind {
            DeclKind::Module(m) if m.abi.is_some() => extern_module(design, id, &mut out),
            DeclKind::Fn(f) => {
                let in_trait = data.parent.is_some_and(|p| matches!(design.decl_data(p).kind, DeclKind::Trait(_)));
                let intrinsic_outside_std = matches!(&f.abi, Some(Lit { kind: LitKind::Str { text, .. }, .. }) if text == "intrinsic") && design.file_data(data.file).krate != design.std_crate;
                let message = match (f.abi.is_some(), f.body.is_some()) {
                    (true, true) => Some("an `extern` function cannot have a body"),
                    (false, false) if !in_trait => Some("this function has no body; only an `extern` function or a trait item may omit it"),
                    (true, false) if intrinsic_outside_std => Some("`extern \"intrinsic\"` is accepted only in the standard library"),
                    _ => None,
                };
                if let (Some(message), Some(name)) = (message, &data.name) {
                    let mut diagnostic = Diagnostic::new("E0125", message.to_string(), name.span, "here");
                    if intrinsic_outside_std && f.body.is_none() {
                        diagnostic = diagnostic.with_help("only the standard library declares intrinsic functions; write a body or use `extern \"C\"`");
                    }
                    out.push((data.file, diagnostic));
                }
            }
            DeclKind::Nature(n) if n.kind == NatureKind::Invalid => {
                if let Some(name) = &data.name {
                    let message = format!("nature `{}` repeats a field, mixes discrete and continuous fields, or has neither `value` nor `potential`", name.text);
                    out.push((data.file, Diagnostic::new("E0201", message, name.span, "invalid nature")));
                }
            }
            _ => {}
        }
    }
    out
}

/// §8.5.7: an extern module lists params only, has no result and no value input.
fn extern_module(design: &Design, module: DeclId, out: &mut Vec<(FileId, Diagnostic)>) {
    let data = design.decl_data(module);
    let DeclKind::Module(m) = &data.kind else { return };
    let mut report = |span, message: String, label: &str| out.push((data.file, Diagnostic::new("E0370", message, span, label.to_string())));
    if let Some(ret) = m.ret {
        report(design.type_node(ret).span, "an `extern` module has no result".to_string(), "result type");
    }
    for child in &data.children {
        let item = design.decl_data(*child);
        match &item.kind {
            DeclKind::GenericParam(_) | DeclKind::Param(_) | DeclKind::Error => {}
            DeclKind::Port(p) if p.kind == PortKind::ValueInput => report(item.span, "an `extern` module has no value input".to_string(), "value input"),
            DeclKind::Port(_) => {}
            other => report(item.span, format!("an `extern` module lists params only, not a `{}`", other.label()), "not a param"),
        }
    }
}

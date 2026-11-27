//! Stage 0 during lowering: `mod b;` finds, reads and lowers its file as the
//! children of the namespace `b`, or reports `E0161` and leaves it empty.

use std::path::PathBuf;

use super::Lowerer;
use super::meta::doc;
use crate::msl::{Diagnostic, Span, parser};
use crate::pom::decl::{DeclKind, NamespaceData, NamespaceOrigin};
use crate::pom::design::SourceFileData;
use crate::pom::ids::DeclId;
use crate::pom::source::mod_candidates;

impl Lowerer<'_> {
    /// Resolve `mod name;` (the namespace `id`, whose item spans `span`).
    pub(crate) fn load_mod(&mut self, id: DeclId, span: Span, name: &str) {
        let candidates = mod_candidates(&self.path, self.is_root, &self.inline, name);
        let found: Vec<(PathBuf, Vec<u8>)> =
            candidates.iter().filter_map(|c| self.provider.read(c).map(|bytes| (c.clone(), bytes))).collect();
        let [first, second] = &candidates;
        let message = match found.as_slice() {
            [] => format!("cannot find module `{name}`: neither `{}` nor `{}` exists", first.display(), second.display()),
            [_, _] => format!("module `{name}` is ambiguous: both `{}` and `{}` exist", first.display(), second.display()),
            [(path, _)] if self.chain.contains(&self.provider.canonical(path)) => {
                format!("the modules form a cycle: `{}` is already being loaded", path.display())
            }
            [(path, bytes)] => {
                let (path, bytes) = (path.clone(), bytes.clone());
                self.load_file(id, path, &bytes);
                return;
            }
            _ => unreachable!("two candidates at most"),
        };
        let diagnostic = Diagnostic::new("E0161", message, span, "module file not loaded");
        self.design.push_diagnostic(self.file, diagnostic);
    }

    /// Parse `bytes` as the file of namespace `id` and lower its items.
    fn load_file(&mut self, id: DeclId, path: PathBuf, bytes: &[u8]) {
        let parsed = parser::SourceParser::parse_bytes(bytes);
        let data = SourceFileData {
            name: path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned()),
            path: Some(path.clone()),
            krate: self.design.file_data(self.file).krate,
            text: String::from_utf8_lossy(bytes).into_owned(),
            ast: parsed.file.clone(),
            root: id,
        };
        let file = self.design.push_file(data);
        for diagnostic in parsed.diagnostics {
            self.design.push_diagnostic(file, diagnostic);
        }
        let decl = self.design.decl_mut(id);
        decl.kind = DeclKind::Namespace(NamespaceData { origin: NamespaceOrigin::External { file: Some(file) } });
        decl.meta.docs.extend(parsed.file.inner_docs.iter().map(|d| doc(d, true, file)));

        let canonical = self.provider.canonical(&path);
        let saved_file = std::mem::replace(&mut self.file, file);
        let saved_path = std::mem::replace(&mut self.path, path);
        let saved_root = std::mem::replace(&mut self.is_root, false);
        let saved_inline = std::mem::take(&mut self.inline);
        self.chain.push(canonical);
        self.within(id, |lw| {
            for item in &parsed.file.items {
                lw.item(item);
            }
        });
        self.chain.pop();
        self.inline = saved_inline;
        self.is_root = saved_root;
        self.path = saved_path;
        self.file = saved_file;
    }
}

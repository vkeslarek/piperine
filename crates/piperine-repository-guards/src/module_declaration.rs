//! The `mod x;` declarations of a file: the edges that say which file a module lives in.

use std::path::{Component, Path, PathBuf};

use syn::{Attribute, Item, Lit, Meta};

/// One `mod x;` declaration (a module whose body is in another file), with the `#[path]` it carries
/// and the inline modules it sits in.
pub struct ModuleDeclaration {
    /// The declared module's name.
    pub name: String,
    /// The value of a `#[path = "..."]` attribute, when there is one.
    pub path_attribute: Option<String>,
    /// The names of the inline `mod a { ... }` blocks the declaration is written inside.
    pub inline_path: Vec<String>,
    /// Whether the declaration, or an inline module around it, carries `#[cfg(test)]`.
    pub cfg_test: bool,
}

impl ModuleDeclaration {
    /// Every file-backed module declaration of a parsed file, inline modules followed.
    pub fn of_file(file: &syn::File) -> Vec<ModuleDeclaration> {
        let mut found = Vec::new();
        Self::collect(&file.items, &[], false, &mut found);
        found
    }

    fn collect(items: &[Item], inline_path: &[String], inherited_test: bool, found: &mut Vec<ModuleDeclaration>) {
        for item in items {
            let Item::Mod(module) = item else { continue };
            let cfg_test = inherited_test || Self::is_cfg_test(&module.attrs);
            match &module.content {
                Some((_, inner)) => {
                    let mut deeper = inline_path.to_vec();
                    deeper.push(module.ident.to_string());
                    Self::collect(inner, &deeper, cfg_test, found);
                }
                None => found.push(ModuleDeclaration {
                    name: module.ident.to_string(),
                    path_attribute: Self::path_attribute_of(&module.attrs),
                    inline_path: inline_path.to_vec(),
                    cfg_test,
                }),
            }
        }
    }

    /// Whether an attribute list holds `#[cfg(test)]` (or a `cfg` of `all(test, ..)`, never of `not(test)`).
    pub fn is_cfg_test(attributes: &[Attribute]) -> bool {
        attributes.iter().any(|attribute| {
            let Meta::List(list) = &attribute.meta else { return false };
            if !list.path.is_ident("cfg") {
                return false;
            }
            let words: Vec<String> = list.tokens.to_string().split(|c: char| !c.is_alphanumeric() && c != '_').map(str::to_string).collect();
            words.iter().any(|w| w == "test") && !words.iter().any(|w| w == "not")
        })
    }

    fn path_attribute_of(attributes: &[Attribute]) -> Option<String> {
        attributes.iter().find_map(|attribute| {
            let Meta::NameValue(pair) = &attribute.meta else { return None };
            if !pair.path.is_ident("path") {
                return None;
            }
            match &pair.value {
                syn::Expr::Lit(literal) => match &literal.lit {
                    Lit::Str(text) => Some(text.value()),
                    _ => None,
                },
                _ => None,
            }
        })
    }

    /// The files this declaration may live in, written in `file`, most likely first.
    pub fn candidates(&self, file: &Path) -> Vec<PathBuf> {
        let directory = file.parent().unwrap_or(Path::new(""));
        let mut inline = PathBuf::new();
        for name in &self.inline_path {
            inline.push(name);
        }
        if let Some(path) = &self.path_attribute {
            return vec![Self::normalized(&directory.join(&inline).join(path))];
        }
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let mut bases = vec![directory.join(&inline)];
        if !matches!(stem, "mod" | "lib" | "main") {
            bases.push(directory.join(stem).join(&inline));
        }
        let mut candidates = Vec::new();
        for base in bases {
            candidates.push(Self::normalized(&base.join(format!("{}.rs", self.name))));
            candidates.push(Self::normalized(&base.join(&self.name).join("mod.rs")));
        }
        candidates
    }

    /// A path with its `.` and `..` components resolved lexically.
    fn normalized(path: &Path) -> PathBuf {
        let mut result = PathBuf::new();
        for component in path.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    result.pop();
                }
                other => result.push(other.as_os_str()),
            }
        }
        result
    }
}

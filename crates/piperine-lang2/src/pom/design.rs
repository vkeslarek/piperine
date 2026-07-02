//! [`Design`] — the POM root, returned by [`SourceFile::elaborate`][crate::parse::SourceFile::elaborate].

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::parse::ast::{BundleDecl, CapabilityDecl, DisciplineDecl, EnumDecl};
use crate::pom::{Function, ImplBlock, Module, OverrideMap, Value};

/// The complete output of elaboration — the POM root.
///
/// Fields are `pub(crate)`; external consumers use the public accessor
/// methods that implement the POM reflection interface
/// (`docs/reflection_api.md`).
#[derive(Debug, Clone)]
pub struct Design {
    pub(crate) modules: HashMap<String, Module>,
    pub(crate) disciplines: HashMap<String, DisciplineDecl>,
    pub(crate) bundles: HashMap<String, BundleDecl>,
    pub(crate) enums: HashMap<String, EnumDecl>,
    pub(crate) capabilities: HashMap<String, CapabilityDecl>,
    pub(crate) functions: HashMap<String, Function>,
    pub(crate) impls: Vec<ImplBlock>,
    pub(crate) consts: HashMap<String, Value>,
    /// Staged parameter overrides — the single mutation surface in POM.
    /// Writing via `set_param()` stages here; re-elaboration consumes.
    pub(crate) overrides: Rc<RefCell<OverrideMap>>,
    /// The top module name, if set by the user or inferred.
    pub(crate) top_module: Option<String>,
}

impl Design {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            disciplines: HashMap::new(),
            bundles: HashMap::new(),
            enums: HashMap::new(),
            capabilities: HashMap::new(),
            functions: HashMap::new(),
            impls: Vec::new(),
            consts: HashMap::new(),
            overrides: Rc::new(RefCell::new(OverrideMap::new())),
            top_module: None,
        }
    }

    // ── POM navigation ────────────────────────────────────────────────────

    /// The elaborated top module, if set.
    pub fn top(&self) -> Option<&Module> {
        self.top_module.as_ref().and_then(|n| self.modules.get(n))
    }

    /// Evaluate a selector path against this design.
    pub fn select<'a>(&'a self, path: &str) -> Result<crate::pom::selection::NodeSelection<'a>, String> {
        let sel = path.parse::<crate::pom::selector::Selector>()?;
        let initial_context = if sel.absolute {
            crate::pom::selection::NodeSelection::new()
        } else {
            // If relative, start from top module
            if let Some(top) = self.top() {
                crate::pom::selection::NodeSelection::from_vec(vec![crate::pom::node::Node::Module(top)])
            } else {
                crate::pom::selection::NodeSelection::new()
            }
        };
        crate::pom::selector::Evaluator::new(self).evaluate(&sel, initial_context)
    }

    /// Set the top module by name.
    pub fn set_top(&mut self, name: &str) {
        self.top_module = Some(name.into());
    }

    /// Look up a module by name.
    pub fn module(&self, name: &str) -> Option<&Module> {
        self.modules.get(name)
    }

    /// Every elaborated (monomorphized) module.
    pub fn modules(&self) -> impl Iterator<Item = &Module> {
        self.modules.values()
    }

    /// Number of elaborated modules.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// Look up a function by name.
    pub fn function(&self, name: &str) -> Option<&Function> {
        self.functions.get(name)
    }

    /// Every discipline declaration.
    pub fn disciplines(&self) -> impl Iterator<Item = (&String, &DisciplineDecl)> {
        self.disciplines.iter()
    }

    /// Look up a discipline declaration by name.
    pub fn discipline(&self, name: &str) -> Option<&DisciplineDecl> {
        self.disciplines.get(name)
    }

    /// Every bundle declaration.
    pub fn bundles(&self) -> impl Iterator<Item = (&String, &BundleDecl)> {
        self.bundles.iter()
    }

    /// Look up a bundle declaration by name.
    pub fn bundle(&self, name: &str) -> Option<&BundleDecl> {
        self.bundles.get(name)
    }

    /// Every enum declaration.
    pub fn enums(&self) -> impl Iterator<Item = (&String, &EnumDecl)> {
        self.enums.iter()
    }

    /// Look up an enum declaration by name.
    pub fn enum_(&self, name: &str) -> Option<&EnumDecl> {
        self.enums.get(name)
    }

    /// Every capability declaration.
    pub fn capabilities(&self) -> impl Iterator<Item = (&String, &CapabilityDecl)> {
        self.capabilities.iter()
    }

    /// Look up a capability declaration by name.
    pub fn capability(&self, name: &str) -> Option<&CapabilityDecl> {
        self.capabilities.get(name)
    }

    /// Every global function.
    pub fn functions(&self) -> impl Iterator<Item = &Function> {
        self.functions.values()
    }

    /// Look up a global constant by name.
    pub fn const_(&self, name: &str) -> Option<&Value> {
        self.consts.get(name)
    }

    /// Every global constant.
    pub fn consts(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.consts.iter()
    }

    /// Every impl block.
    pub fn impls(&self) -> &[ImplBlock] { &self.impls }

    // ── Staging layer ─────────────────────────────────────────────────────

    /// Stage a parameter override. Does NOT mutate the elaborated design —
    /// a subsequent re-elaboration consumes the override purely.
    pub fn set_param(&self, path: &str, param: &str, value: Value) {
        self.overrides.borrow_mut().set(path, param, value);
    }

    /// Look up a staged override.
    pub fn get_override(&self, path: &str, param: &str) -> Option<Value> {
        self.overrides.borrow().get(path, param).cloned()
    }

    /// True if any overrides are staged.
    pub fn has_overrides(&self) -> bool {
        !self.overrides.borrow().is_empty()
    }

    /// Clear all staged overrides.
    pub fn clear_overrides(&self) {
        self.overrides.borrow_mut().clear();
    }

    // ── Internal access (pub(crate)) ──────────────────────────────────────

    /// Mutable access to the modules map. For internal elaboration use.
    pub(crate) fn modules_map_mut(&mut self) -> &mut HashMap<String, Module> {
        &mut self.modules
    }
    /// Mutable access to the disciplines map. For internal elaboration use.
    pub(crate) fn disciplines_map_mut(&mut self) -> &mut HashMap<String, DisciplineDecl> {
        &mut self.disciplines
    }
    /// Mutable access to the bundles map. For internal elaboration use.
    pub(crate) fn bundles_map_mut(&mut self) -> &mut HashMap<String, BundleDecl> {
        &mut self.bundles
    }
    /// Mutable access to the enums map. For internal elaboration use.
    pub(crate) fn enums_map_mut(&mut self) -> &mut HashMap<String, EnumDecl> {
        &mut self.enums
    }
    /// Mutable access to the capabilities map. For internal elaboration use.
    pub(crate) fn capabilities_map_mut(&mut self) -> &mut HashMap<String, CapabilityDecl> {
        &mut self.capabilities
    }
    /// Mutable access to the functions map. For internal elaboration use.
    pub(crate) fn functions_map_mut(&mut self) -> &mut HashMap<String, Function> {
        &mut self.functions
    }
    /// Mutable access to the impl blocks vec. For internal elaboration use.
    pub(crate) fn impls_vec_mut(&mut self) -> &mut Vec<ImplBlock> {
        &mut self.impls
    }
    /// Mutable access to the consts map. For internal elaboration use.
    pub(crate) fn consts_map_mut(&mut self) -> &mut HashMap<String, Value> {
        &mut self.consts
    }

    /// Insert a module by name. Used internally by the elaborator and by
    /// the digital interpreter bridge to build synthetic modules for
    /// digital-only test scenarios.
    pub(crate) fn insert_module(&mut self, name: String, module: Module) {
        self.modules.insert(name, module);
    }
}

impl Default for Design {
    fn default() -> Self { Self::new() }
}
